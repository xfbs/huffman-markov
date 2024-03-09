use crate::{markov::Markov, util::buffered_windows};
use bitstream_io::{BigEndian, BitWrite, BitWriter, Endianness};
use bitvec::prelude::*;
use std::{
    borrow::Borrow,
    cmp::Reverse,
    collections::{BinaryHeap, HashMap},
    io::{Result as IoResult, Write},
};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Node {
    Leaf(u8),
    Node { left: Box<Node>, right: Box<Node> },
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct WeightedNode {
    weight: usize,
    node: Node,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct WeightedItem<T = u8> {
    pub weight: usize,
    pub item: T,
}

impl Node {
    fn new(items: impl Iterator<Item = WeightedItem>) -> Option<Self> {
        let mut heap: BinaryHeap<Reverse<WeightedNode>> = items
            .map(|item| {
                Reverse(WeightedNode {
                    weight: item.weight,
                    node: Node::Leaf(item.item),
                })
            })
            .collect();

        while heap.len() > 1 {
            let left = heap.pop().unwrap().0;
            let right = heap.pop().unwrap().0;
            let node = WeightedNode {
                // FIXME: saturating add?
                weight: left.weight.saturating_add(right.weight),
                node: Node::Node {
                    left: left.node.into(),
                    right: right.node.into(),
                },
            };
            heap.push(Reverse(node));
        }

        let root = heap.pop()?.0;

        Some(root.node)
    }

    fn iter(&self, mut prefix: BitVec) -> Box<dyn Iterator<Item = (BitVec, u8)> + '_> {
        match self {
            Self::Leaf(byte) => {
                prefix.reverse();
                Box::new(std::iter::once((prefix, *byte)))
            }
            Self::Node { left, right } => {
                prefix.push(false);
                let left = left.iter(prefix.clone());
                prefix.pop();
                prefix.push(true);
                let right = right.iter(prefix);
                Box::new(left.chain(right))
            }
        }
    }

    fn encoding(&self) -> HashMap<u8, BitBox> {
        self.iter(Default::default())
            .map(|(bits, byte)| (byte, bits.into()))
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Decoder {
    pub depth: usize,
    pub trees: HashMap<Box<[u8]>, Node>,
}

impl Decoder {
    pub fn new(markov: &Markov) -> Self {
        let mut huffman = Decoder {
            depth: markov.len(),
            trees: Default::default(),
        };
        for (prefix, items) in markov.iter_prefix() {
            huffman
                .trees
                .insert(prefix.into(), Node::new(items.into_iter()).unwrap());
        }
        huffman
    }

    pub fn encoder(&self) -> Encoder {
        Encoder::new(self)
    }

    fn decoder(&self, prefix: &[u8]) -> () {}
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Encoder {
    pub depth: usize,
    pub prefixes: HashMap<Box<[u8]>, HashMap<u8, BitBox>>,
}

impl Encoder {
    fn new(decoder: &Decoder) -> Self {
        Encoder {
            depth: decoder.depth,
            prefixes: decoder
                .trees
                .iter()
                .map(|(prefix, node)| (prefix.clone(), node.encoding()))
                .collect(),
        }
    }

    fn encode(&self, prefix: &[u8], byte: u8) -> Option<&BitSlice> {
        Some(self.prefixes.get(prefix)?.get(&byte)?.as_bitslice())
    }

    pub fn writer<W: Write>(&self, writer: W) -> Writer<&Self, W> {
        Writer::new(self, writer)
    }
}

pub struct Writer<H: Borrow<Encoder>, W: Write, E: Endianness = BigEndian> {
    buffer: Vec<u8>,
    encoder: H,
    writer: BitWriter<W, E>,
}

impl<H: Borrow<Encoder>, W: Write> Writer<H, W> {
    fn new(encoder: H, writer: W) -> Self {
        Self {
            buffer: vec![],
            encoder,
            writer: BitWriter::new(writer),
        }
    }
}

impl<H: Borrow<Encoder>, W: Write, E: Endianness> Write for Writer<H, W, E> {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        let encoder = self.encoder.borrow();
        buffered_windows(encoder.depth, &mut self.buffer, buf, |window| {
            let prefix = &window[0..window.len() - 1];
            let byte = window[window.len() - 1];
            let slice = encoder.encode(prefix, byte).unwrap();
            for bit in slice.iter() {
                self.writer.write_bit(*bit);
            }
            Ok(()) as IoResult<()>
        })
        .unwrap();
        Ok(buf.len())
    }

    fn flush(&mut self) -> IoResult<()> {
        for _ in 0..7 {
            self.writer.write_bit(false);
        }
        self.writer.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::collections::BTreeMap;
    use test_strategy::proptest;

    #[proptest]
    fn test_node(#[filter(!#items.is_empty())] items: BTreeMap<u8, usize>) {
        let node = Node::new(items.iter().map(|(item, weight)| WeightedItem {
            item: *item,
            weight: *weight,
        }))
        .unwrap();
        let encoder = node.encoding();

        for (byte, weight) in items.iter() {
            prop_assert!(encoder.get(byte).is_some());
        }
    }
}
