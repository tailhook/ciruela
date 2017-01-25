use argparse::{ArgumentParser};


pub struct GlobalOptions {
}

impl GlobalOptions {
    pub fn new() -> GlobalOptions {
        GlobalOptions {
        }
    }
    pub fn define(&mut self, _ap: &mut ArgumentParser) {

    }
}

