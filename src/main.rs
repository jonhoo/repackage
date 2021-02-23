fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    if std::env::args().len() == 1 {
        anyhow::bail!(
            "usage: {} <.crate> <new-name>",
            std::env::args().next().unwrap()
        );
    }

    repackage::dot_crate(&args.next().unwrap(), None, &args.next().unwrap())
}
