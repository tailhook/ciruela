use futures::Stream;

use proto::request::{PacketStream, Registry};


pub trait StreamExt: Stream {
    fn packetize(self, registry: &Registry)
        -> PacketStream<Self>
        where Self: Sized
    {
        PacketStream::new(self, registry)
    }
}

impl<T: Stream> StreamExt for T {}
