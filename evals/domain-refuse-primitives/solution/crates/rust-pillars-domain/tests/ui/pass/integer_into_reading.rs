use rust_pillars_domain::Reading;

fn main() {
    let r: Reading =
        <Reading as zerocopy::FromBytes>::read_from_bytes(&215_i16.to_le_bytes()).unwrap();
    let _ = r;
}
