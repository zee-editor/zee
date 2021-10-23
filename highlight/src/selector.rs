use fnv::FnvHashMap;
use nom::{
    branch::alt,
    bytes::complete::{escaped, is_not, tag, take_while1},
    character::complete::{char, digit1, multispace0, one_of},
    combinator::opt,
    error::context,
    multi::separated_list,
    sequence::{delimited, preceded},
    IResult,
};
use serde_derive::{Deserialize, Serialize};
use smallvec::SmallVec;
use std::convert::TryFrom;

use crate::error::{Error, Result};

// Unfortunately, (and surprisingly) it seems that tree-sitter grammers map
// multiple node kind "ids" to the same node kind string. The highlighting rules
// refer to strings. To apply rules and avoid string matching, we have to first
// map the node kind ids of a grammar to a unique set.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Hash)]
pub struct SelectorNodeId(pub(crate) u16);

pub type NthChild = i16;

const NTH_CHILD_ANY: NthChild = -1;

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct Selector {
    node_kinds: SmallVec<[SelectorNodeId; 4]>,
    nth_children: SmallVec<[NthChild; 4]>,
}

impl Selector {
    #[inline]
    pub(crate) fn node_kinds(&self) -> &[SelectorNodeId] {
        self.node_kinds.as_slice()
    }

    #[inline]
    pub(crate) fn nth_children(&self) -> &[NthChild] {
        self.nth_children.as_slice()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SelectorRaw {
    pub node_selectors: Vec<NodeSelectorRaw>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NodeSelectorRaw {
    pub node_kind: String,
    pub nth_child: Option<usize>,
}

pub(crate) fn map_node_kind_names(
    node_kind_id_for_name: &FnvHashMap<&str, SelectorNodeId>,
    selector: SelectorRaw,
) -> Result<Selector> {
    let mut node_kinds = SmallVec::new();
    let mut nth_children = SmallVec::new();
    selector
        .node_selectors
        .into_iter()
        .rev()
        .try_for_each(
            |NodeSelectorRaw {
                 node_kind,
                 nth_child,
             }| {
                node_kind_id_for_name
                    .get(node_kind.as_str())
                    .map(|&node_kind| {
                        node_kinds.push(node_kind);
                        nth_children.push(
                            nth_child
                                .map(|nth_child| {
                                    i16::try_from(nth_child).expect("nth_child to fit into i16")
                                })
                                .unwrap_or(NTH_CHILD_ANY),
                        );
                    })
                    .ok_or(Error::NodeKindNotFound(node_kind))
            },
        )
        .map(|_| Selector {
            node_kinds,
            nth_children,
        })
}

pub(crate) fn parse(input: &str) -> Result<Vec<SelectorRaw>> {
    selectors(input)
        .map(|(_, selectors)| {
            selectors
                .into_iter()
                .filter(|selector| !selector.node_selectors.is_empty())
                .collect()
        })
        .map_err(|_| Error::SelectorSyntax)
}

fn selectors(input: &str) -> IResult<&str, Vec<SelectorRaw>> {
    separated_list(preceded(multispace0, char(',')), selector)(input)
}

fn selector(input: &str) -> IResult<&str, SelectorRaw> {
    context(
        "selector",
        separated_list(preceded(multispace0, char('>')), node_selector),
    )(input)
    .map(|(remaining, node_selectors)| (remaining, SelectorRaw { node_selectors }))
}

fn node_selector(input: &str) -> IResult<&str, NodeSelectorRaw> {
    let (input, _) = multispace0(input)?;
    let (input, identifier_str) = identifier(input)?;
    let (input, nth) = opt(delimited(tag(":nth-child("), digit1, tag(")")))(input)?;
    Ok((
        input,
        NodeSelectorRaw {
            node_kind: identifier_str.into(),
            nth_child: nth
                .map_or(Ok(None), |nth| nth.parse::<usize>().map(Some))
                .unwrap(),
        },
    ))
}

fn identifier(input: &str) -> IResult<&str, &str> {
    alt((
        delimited(
            char('"'),
            escaped(is_not(r#"\""#), '\\', one_of(r#"\""#)),
            char('"'),
        ),
        take_while1(|c: char| c.is_alphanumeric() || c == '_' || c == '-'),
    ))(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parent_child_with_nth_child() {
        let selector_str = "pair > string:nth-child(0)";
        let expected = vec![SelectorRaw {
            node_selectors: vec![
                NodeSelectorRaw {
                    node_kind: "pair".into(),
                    nth_child: None,
                },
                NodeSelectorRaw {
                    node_kind: "string".into(),
                    nth_child: Some(0),
                },
            ],
        }];
        assert_eq!(Ok(("", expected)), selectors(selector_str));
    }

    #[test]
    fn test_single_identifier() {
        let selector_str = "      identifier";
        let expected = vec![SelectorRaw {
            node_selectors: vec![NodeSelectorRaw {
                node_kind: "identifier".into(),
                nth_child: None,
            }],
        }];
        assert_eq!(Ok(("", expected)), selectors(selector_str));
    }

    #[test]
    fn test_multiple_selectors() {
        let selector_str = "Foo > bar, foo:nth-child(7), bar";
        let expected = vec![
            SelectorRaw {
                node_selectors: vec![
                    NodeSelectorRaw {
                        node_kind: "Foo".into(),
                        nth_child: None,
                    },
                    NodeSelectorRaw {
                        node_kind: "bar".into(),
                        nth_child: None,
                    },
                ],
            },
            SelectorRaw {
                node_selectors: vec![NodeSelectorRaw {
                    node_kind: "foo".into(),
                    nth_child: Some(7),
                }],
            },
            SelectorRaw {
                node_selectors: vec![NodeSelectorRaw {
                    node_kind: "bar".into(),
                    nth_child: None,
                }],
            },
        ];
        assert_eq!(Ok(("", expected)), selectors(selector_str));
    }

    #[test]
    fn test_quoted_node_selector() {
        let selector_str = "\"unnamed\"";
        assert_eq!(
            Ok((
                "",
                NodeSelectorRaw {
                    node_kind: "unnamed".into(),
                    nth_child: None
                }
            )),
            node_selector(selector_str)
        );
        let selector_str = "\"unnamed\":nth-child(11)";
        assert_eq!(
            Ok((
                "",
                NodeSelectorRaw {
                    node_kind: "unnamed".into(),
                    nth_child: Some(11)
                }
            )),
            node_selector(selector_str)
        );
    }

    #[test]
    fn test_multiple_selectors_with_quoted_identifiers() {
        let selector_str = "\"unnamed\", other_identifier, \"&\" > \"abc\":nth-child(0)";
        let expected = vec![
            SelectorRaw {
                node_selectors: vec![NodeSelectorRaw {
                    node_kind: "unnamed".into(),
                    nth_child: None,
                }],
            },
            SelectorRaw {
                node_selectors: vec![NodeSelectorRaw {
                    node_kind: "other_identifier".into(),
                    nth_child: None,
                }],
            },
            SelectorRaw {
                node_selectors: vec![
                    NodeSelectorRaw {
                        node_kind: "&".into(),
                        nth_child: None,
                    },
                    NodeSelectorRaw {
                        node_kind: "abc".into(),
                        nth_child: Some(0),
                    },
                ],
            },
        ];
        assert_eq!(Ok(("", expected)), selectors(selector_str));
    }

    #[test]
    fn test_escaped_quote() {
        let selector_str = r#""\"""#;
        assert_eq!(Ok(("", r#"\""#)), identifier(selector_str));
    }

    // #[test]
    // fn test_backslash_identifier() {
    //     assert_eq!(Ok(("", r#"\"#)), identifier(r#""\\""#));
    // }

    #[test]
    fn test_escaped() {
        let selector_str = r#" "\""  ,",":nth-child(7),"|","\"">      test> "x":nth-child(1)"#;
        let expected = vec![
            SelectorRaw {
                node_selectors: vec![NodeSelectorRaw {
                    node_kind: "\\\"".into(),
                    nth_child: None,
                }],
            },
            SelectorRaw {
                node_selectors: vec![NodeSelectorRaw {
                    node_kind: ",".into(),
                    nth_child: Some(7),
                }],
            },
            SelectorRaw {
                node_selectors: vec![NodeSelectorRaw {
                    node_kind: "|".into(),
                    nth_child: None,
                }],
            },
            SelectorRaw {
                node_selectors: vec![
                    NodeSelectorRaw {
                        node_kind: "\\\"".into(),
                        nth_child: None,
                    },
                    NodeSelectorRaw {
                        node_kind: "test".into(),
                        nth_child: None,
                    },
                    NodeSelectorRaw {
                        node_kind: "x".into(),
                        nth_child: Some(1),
                    },
                ],
            },
        ];
        assert_eq!(Ok(("", expected)), selectors(selector_str));
    }

    #[test]
    fn test_trailing_comma_doesnt_include_selector() {
        let selector_str = "\"as\",\n\"*\",\n\"&\",";
        let expected = vec![
            SelectorRaw {
                node_selectors: vec![NodeSelectorRaw {
                    node_kind: "as".into(),
                    nth_child: None,
                }],
            },
            SelectorRaw {
                node_selectors: vec![NodeSelectorRaw {
                    node_kind: "*".into(),
                    nth_child: None,
                }],
            },
            SelectorRaw {
                node_selectors: vec![NodeSelectorRaw {
                    node_kind: "&".into(),
                    nth_child: None,
                }],
            },
        ];
        assert_eq!(Ok(expected), parse(selector_str));
    }
}
