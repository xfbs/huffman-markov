# Huffman-Markov Compression

Compressiong using markov chains and huffman coding. This project serves no
practical value, apart from me trying to prove to myself that I can implement a
compression algorithm that is not entirely useless in Rust on an afternoon.

I had the idea that it would be interesting to combine Markov-chains with
Huffman encoding, such that you end up with a different Huffman tree for
every possible prefix. This means that the huffman encoding is context-sensitive
and it can "recognize" common sequences more easily.

However, while I came up with this on my own, it is not a novel idea. I have
linked some resources in the *Reading* section for more context.

## Algorithm







## Reading

[Markov-Huffman-Coding](https://github.com/jeremy-rifkin/Markov-Huffman-Coding)

## License

MIT.
