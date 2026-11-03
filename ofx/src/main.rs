//! The OFX importer, as a PennyChest plugin.

use pennychest_plugin::Plugin;

fn main() {
    Plugin::new("ofx", env!("CARGO_PKG_VERSION"))
        .importer(ofx::Ofx)
        .serve();
}
