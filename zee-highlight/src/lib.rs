mod error;
mod selector;

use fnv::FnvHashMap;
use serde_derive::{self, Deserialize, Serialize};
use std::{cmp, collections::HashMap, convert::TryFrom};

use error::Result;
use tree_sitter::Language;

use crate::selector::{map_node_kind_names, Selector};

pub use crate::selector::SelectorNodeId;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct HighlightRules {
    name: String,
    node_id_to_selector_id: FnvHashMap<u16, SelectorNodeId>,

    #[serde(default)]
    rules: Vec<HighlightRule>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct HighlightRule {
    selectors: Vec<Selector>,
    scope: ScopePattern,
}

impl HighlightRules {
    #[inline]
    pub fn get_selector_node_id(&self, node_kind_id: u16) -> SelectorNodeId {
        self.node_id_to_selector_id
            .get(&node_kind_id)
            .copied()
            .unwrap_or_else(|| {
                SelectorNodeId(u16::try_from(self.node_id_to_selector_id.len()).unwrap())
            })
    }

    #[inline]
    pub fn matches(
        &self,
        node_stack: &[SelectorNodeId],
        nth_children: &[u16],
        content: &str,
    ) -> Option<&Scope> {
        if node_stack.is_empty() {
            return None;
        }

        let mut distance_to_match = std::usize::MAX;
        let mut num_nodes_match = 0;
        let mut scope_pattern = None;
        for rule in self.rules.iter() {
            let rule_scope = match rule.scope.matches(content) {
                Some(scope) => scope,
                None => continue,
            };

            for selector in rule.selectors.iter() {
                let selector_node_kinds = selector.node_kinds();
                let selector_nth_children = selector.nth_children();

                // eprintln!("NST {:?} {:?}", node_stack, nth_children);
                // eprintln!("SEL {:?} {:?}", selector_node_kinds, selector_nth_children);

                assert!(!selector_node_kinds.is_empty());
                if selector_node_kinds.len() > node_stack.len() {
                    continue;
                }

                // TODO: Are for loops over inclusive ranges slow?
                for start in 0..=cmp::min(
                    node_stack.len().saturating_sub(selector_node_kinds.len()),
                    distance_to_match,
                ) {
                    let span_range = || start..start + selector_node_kinds.len();

                    // Does the selector match the current node and its ancestors?
                    if selector_node_kinds
                        != &node_stack[start..(start + selector_node_kinds.len())]
                    {
                        continue;
                    }

                    // Are the `nth-child` constrains also satisfied?
                    let nth_child_not_satisfied = selector_nth_children
                        .iter()
                        .zip(nth_children[span_range()].iter())
                        .any(|(&nth_child_selector, &node_sibling_index)| {
                            nth_child_selector >= 0
                                && nth_child_selector as u16 != node_sibling_index
                        });
                    if nth_child_not_satisfied {
                        continue;
                    }

                    // Is the selector more specific than the most specific
                    // match we've found so far?
                    if start == distance_to_match && num_nodes_match > selector_node_kinds.len() {
                        break;
                    }

                    assert!(start <= distance_to_match);
                    // eprintln!(
                    //     "!!D {} -> {} | N {} -> {}",
                    //     distance_to_match,
                    //     start,
                    //     num_nodes_match,
                    //     selector_node_kinds.len()
                    // );

                    distance_to_match = start;
                    num_nodes_match = selector_node_kinds.len();
                    scope_pattern = Some(rule_scope);
                    break;
                }
            }
        }

        scope_pattern
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RawHighlightRules {
    name: String,

    #[serde(default)]
    pub scopes: HashMap<String, ScopePattern>,
}

impl RawHighlightRules {
    fn compile(self, language: Language) -> Result<HighlightRules> {
        let (node_name_to_selector_id, node_id_to_selector_id) =
            build_node_to_selector_id_maps(language);
        let RawHighlightRules { name, scopes } = self;

        scopes
            .into_iter()
            .map(|(selector_str, scope)| {
                let selectors = selector::parse(&selector_str)?;
                let selectors = selectors
                    .into_iter()
                    .map(|selector| map_node_kind_names(&node_name_to_selector_id, selector))
                    .collect::<Result<Vec<_>>>()?;
                Ok(HighlightRule { selectors, scope })
            })
            .collect::<Result<Vec<_>>>()
            .map(|rules| HighlightRules {
                name,
                rules,
                node_id_to_selector_id,
            })
    }
}

fn build_node_to_selector_id_maps(
    language: Language,
) -> (
    FnvHashMap<&'static str, SelectorNodeId>,
    FnvHashMap<u16, SelectorNodeId>,
) {
    let mut node_name_to_selector_id =
        FnvHashMap::with_capacity_and_hasher(language.node_kind_count(), Default::default());
    let mut node_id_to_selector_id =
        FnvHashMap::with_capacity_and_hasher(language.node_kind_count(), Default::default());

    let node_id_range =
        0..u16::try_from(language.node_kind_count()).expect("node_kind_count() should fit in u16");
    for node_id in node_id_range {
        let node_name = language
            .node_kind_for_id(node_id)
            .expect("node kind available for node_id in range");
        let next_selector_id =
            SelectorNodeId(u16::try_from(node_name_to_selector_id.len()).unwrap());
        let selector_id = node_name_to_selector_id
            .entry(node_name)
            .or_insert_with(|| next_selector_id);
        node_id_to_selector_id.insert(node_id, *selector_id);
    }

    // log::debug!(
    //     "NKC: {}, name->sid: {}, nid->sid: {}",
    //     language.node_kind_count(),
    //     node_name_to_selector_id.len(),
    //     node_id_to_selector_id.len(),
    // );

    (node_name_to_selector_id, node_id_to_selector_id)
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ScopePattern {
    All(Scope),
    Exact {
        exact: String,
        scopes: Scope,
    },
    Regex {
        #[serde(rename = "match")]
        regex: Regex,
        scopes: Scope,
    },
    Vec(Vec<ScopePattern>),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Regex(#[serde(with = "serde_regex")] regex::Regex);

impl Regex {
    fn is_match(&self, text: &str) -> bool {
        self.0.is_match(text)
    }
}

impl PartialEq for Regex {
    fn eq(&self, other: &Self) -> bool {
        self.0.as_str() == other.0.as_str()
    }
}

impl ScopePattern {
    fn matches(&self, content: &str) -> Option<&Scope> {
        match self {
            ScopePattern::All(ref scopes) => Some(scopes),
            ScopePattern::Exact {
                ref exact,
                ref scopes,
            } if exact.as_str() == content => Some(scopes),
            ScopePattern::Regex {
                ref regex,
                ref scopes,
            } if regex.is_match(content) => Some(scopes),
            ScopePattern::Vec(ref scope_patterns) => {
                for scope_pattern in scope_patterns.iter() {
                    let maybe_scope = scope_pattern.matches(content);
                    if maybe_scope.is_some() {
                        return maybe_scope;
                    }
                }
                None
            }
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Scope(pub String);

pub fn parse_rules_unwrap(language: Language, source: &str) -> HighlightRules {
    let raw_rules =
        serde_json::from_str::<RawHighlightRules>(source).expect("valid json file for rules");
    let name = format!("valid rules for {}", raw_rules.name);
    raw_rules.compile(language).expect(&name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use maplit::hashmap;

    #[test]
    fn deserialize_no_scopes() {
        let style_str = r#"{"name": "Rust"}"#;
        let expected = RawHighlightRules {
            name: "Rust".into(),
            scopes: Default::default(),
        };
        let actual: RawHighlightRules = serde_json::from_str(style_str).expect("valid json");
        assert_eq!(expected.name, actual.name);
    }

    #[test]
    fn deserialize_all_scope_types() {
        let style_str = r#"{
            "name": "Rust",
            "scopes": {
                "type_identifier": "support.type",
                "\"let\"": {"exact": "let", "scopes": "keyword.control" }
            }
        }"#;
        let expected = RawHighlightRules {
            name: "Rust".into(),
            scopes: hashmap! {
                "type_identifier".into() => ScopePattern::All(Scope("support.type".into())),
                "\"let\"".into() => ScopePattern::Exact {
                    exact: "let".into(),
                    scopes: Scope("keyword.control".into())
                },
            },
        };
        let actual: RawHighlightRules = serde_json::from_str(style_str).expect("valid json");
        assert_eq!(expected.name, actual.name);
        assert_eq!(expected.scopes, actual.scopes);
    }
}
