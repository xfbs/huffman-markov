use crate::{
    huffman::{Decoder, Encoder, WeightedItem},
    util::buffered_windows,
};
use std::{
    borrow::BorrowMut,
    collections::BTreeMap,
    io::{Result as IoResult, Write},
};

pub type Map<K, V> = BTreeMap<K, V>;

#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    Leaf(usize),
    Node(Map<u8, Self>),
}

impl Node {
    fn node_mut(&mut self) -> Option<&mut Map<u8, Self>> {
        match self {
            Node::Node(node) => Some(node),
            Node::Leaf(_) => None,
        }
    }

    fn leaf(&self) -> Option<usize> {
        match self {
            Node::Node(_) => None,
            Node::Leaf(weight) => Some(*weight),
        }
    }

    fn node(&self) -> Option<&Map<u8, Self>> {
        match self {
            Node::Node(node) => Some(node),
            Node::Leaf(_) => None,
        }
    }

    fn iter(&self, prefix: Vec<u8>) -> Box<dyn Iterator<Item = (Vec<u8>, usize)> + '_> {
        match self {
            Self::Leaf(weight) => Box::new(std::iter::once((prefix, *weight))),
            Self::Node(nodes) => Box::new(nodes.iter().flat_map(move |(byte, node)| {
                let mut prefix = prefix.clone();
                prefix.push(*byte);
                node.iter(prefix)
            })),
        }
    }

    fn iter_prefix(
        &self,
        prefix: Vec<u8>,
        length: usize,
    ) -> Box<dyn Iterator<Item = (Vec<u8>, Vec<WeightedItem>)> + '_> {
        if length == 0 {
            let items = self
                .node()
                .unwrap()
                .iter()
                .map(|(byte, node)| WeightedItem {
                    item: *byte,
                    weight: node.leaf().unwrap(),
                })
                .collect();
            Box::new(std::iter::once((prefix, items)))
        } else {
            Box::new(self.node().unwrap().iter().flat_map(move |(byte, node)| {
                let mut prefix = prefix.clone();
                prefix.push(*byte);
                node.iter_prefix(prefix, length - 1)
            }))
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Markov {
    depth: usize,
    root: Node,
}

#[derive(thiserror::Error, Debug)]
#[error("sequence length mismatch")]
pub struct SequenceLengthError;

impl Markov {
    pub fn new(depth: usize) -> Self {
        Markov {
            depth,
            root: Node::Node(Default::default()),
        }
    }

    pub fn iter(&self) -> Box<dyn Iterator<Item = (Vec<u8>, usize)> + '_> {
        self.root.iter(vec![])
    }

    pub fn iter_prefix(&self) -> Box<dyn Iterator<Item = (Vec<u8>, Vec<WeightedItem>)> + '_> {
        self.root.iter_prefix(vec![], self.depth - 1)
    }

    pub fn len(&self) -> usize {
        self.depth
    }

    pub fn insert(&mut self, sequence: &[u8], weight: usize) -> Result<usize, SequenceLengthError> {
        if sequence.len() != self.depth {
            return Err(SequenceLengthError);
        }

        let leaf =
            sequence[..]
                .into_iter()
                .enumerate()
                .fold(&mut self.root, |node, (index, key)| {
                    let default = if index < (self.depth - 1) {
                        Node::Node(Default::default())
                    } else {
                        Node::Leaf(Default::default())
                    };
                    node.node_mut().unwrap().entry(*key).or_insert(default)
                });

        let count = match leaf {
            Node::Leaf(count) => {
                *count = count.saturating_add(weight);
                *count
            }
            Node::Node(_) => unreachable!(),
        };

        Ok(count)
    }

    pub fn get(&self, sequence: &[u8]) -> Result<Option<&Node>, SequenceLengthError> {
        if sequence.len() != self.depth {
            return Err(SequenceLengthError);
        }

        let result = sequence
            .into_iter()
            .fold(Some(&self.root), |node, key| match node? {
                Node::Node(node) => node.get(key),
                Node::Leaf(_) => None,
            });

        Ok(result)
    }

    pub fn writer(&mut self) -> Writer<&mut Self> {
        Writer::new(self)
    }

    pub fn into_writer(self) -> Writer<Self> {
        Writer::new(self)
    }

    pub fn encoder(&self) -> Encoder {
        self.decoder().encoder()
    }

    pub fn decoder(&self) -> Decoder {
        Decoder::new(self)
    }
}

const DEFAULT_WEIGHT: usize = 1;

pub trait SequenceWriter {
    fn len(&self) -> usize;
    fn write(&mut self, sequence: &[u8]) -> Result<(), SequenceLengthError>;
}

impl<T: BorrowMut<Markov>> SequenceWriter for T {
    fn len(&self) -> usize {
        Markov::len(self.borrow())
    }

    fn write(&mut self, sequence: &[u8]) -> Result<(), SequenceLengthError> {
        Markov::insert(self.borrow_mut(), sequence, DEFAULT_WEIGHT).map(|_| ())
    }
}

#[derive(Debug, Clone)]
pub struct Writer<W: SequenceWriter> {
    writer: W,
    buffer: Vec<u8>,
}

impl<W: SequenceWriter> Writer<W> {
    pub fn new(sequence_writer: W) -> Self {
        Writer {
            writer: sequence_writer,
            buffer: vec![],
        }
    }

    pub fn write(&mut self, input: &[u8]) {
        buffered_windows(self.writer.len(), &mut self.buffer, input, |window| {
            self.writer.write(window)
        })
        .unwrap();
    }

    pub fn finish(self) -> W {
        self.writer
    }
}

impl<W: SequenceWriter> Write for Writer<W> {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        Writer::write(self, buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> IoResult<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use test_strategy::{proptest, Arbitrary};

    macro_rules! test_markov_insert {
        ($name:ident, $len:expr) => {
            #[proptest]
            fn $name(sequences: Vec<([u8; $len], usize)>) {
                let mut markov = Markov::new($len);

                for (sequence, weight) in &sequences {
                    markov.insert(&sequence[..], *weight).unwrap();
                }

                for (sequence, weight) in &sequences {
                    let node = markov.get(&sequence[..]).unwrap().unwrap();
                    let count = match node {
                        Node::Leaf(count) => *count,
                        _ => unreachable!(),
                    };
                    assert!(count >= *weight);
                }
            }
        };
    }

    #[derive(Arbitrary, Debug, Clone, Copy, PartialEq)]
    pub struct Length(#[strategy(1usize..5)] usize);

    impl std::ops::Deref for Length {
        type Target = usize;
        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    test_markov_insert!(test_markov1_insert, 1);
    test_markov_insert!(test_markov2_insert, 2);
    test_markov_insert!(test_markov3_insert, 3);
    test_markov_insert!(test_markov4_insert, 4);
    test_markov_insert!(test_markov5_insert, 5);

    #[proptest]
    fn test_writer(inputs: Vec<Vec<u8>>, length: Length) {
        let markov_writer = {
            let mut writer = Markov::new(*length).into_writer();
            inputs.iter().for_each(|input| writer.write(&input));
            writer.finish()
        };

        let markov_full = {
            let mut markov = Markov::new(*length);
            let input = inputs.into_iter().fold(Vec::new(), |mut vec, mut segment| {
                vec.append(&mut segment);
                vec
            });
            for window in input.windows(*length) {
                markov.insert(window, DEFAULT_WEIGHT).unwrap();
            }
            markov
        };

        prop_assert_eq!(markov_writer, markov_full);
    }
}
