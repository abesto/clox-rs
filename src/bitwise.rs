pub fn get_4_bytes(v: usize) -> (u8, u8, u8, u8) {
    (
        ((v & 0xff000000) >> 24) as u8,
        ((v & 0x00ff0000) >> 16) as u8,
        ((v & 0x0000ff00) >> 8) as u8,
        (v & 0x000000ff) as u8,
    )
}
