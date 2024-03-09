pub fn buffered_windows<T: Clone, E>(
    window_size: usize,
    buffer: &mut Vec<T>,
    input: &[T],
    mut write: impl FnMut(&[T]) -> Result<(), E>,
) -> Result<(), E> {
    // if input is empty, we don't need to do anything.
    if input.is_empty() {
        return Ok(());
    }

    // if the buffer is not filled, we fill it first.
    if buffer.len() < (window_size - 1) {
        buffer.push(input[0].clone());
        return buffered_windows(window_size, buffer, &input[1..], write);
    }

    // first, write the first n chars to fill the buffer.
    let count = input.len().min(buffer.len());
    buffer.extend(input[0..count].into_iter().cloned());
    for window in buffer.windows(window_size) {
        write(window)?;
    }
    buffer.truncate(buffer.len() - count);

    // next, write whatever is in our data
    for window in input.windows(window_size) {
        write(window)?;
    }

    // finally, set buffer to last n bytes
    *buffer = input
        .iter()
        .rev()
        .chain(buffer.iter().rev())
        .take(buffer.len())
        .cloned()
        .collect();
    buffer.reverse();

    Ok(())
}
