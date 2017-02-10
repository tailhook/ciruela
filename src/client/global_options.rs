use argparse::{ArgumentParser, Parse};


pub struct GlobalOptions {
    pub destination_port: u16,
}

impl GlobalOptions {
    pub fn new() -> GlobalOptions {
        GlobalOptions {
            destination_port: 24783,
        }
    }
    pub fn define<'x, 'y>(&'x mut self, ap: &'y mut ArgumentParser<'x>) {
        ap.refer(&mut self.destination_port)
            .add_option(&["--port"], Parse, "
                Port to use for connecting to daemons (default is 24783)
            ");
    }
}

