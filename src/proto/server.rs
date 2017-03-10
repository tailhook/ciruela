use serde_cbor::ser::Serializer as Cbor;
use serde::Serialize;
use tk_http::websocket::Packet;

use proto::RESPONSE;


pub fn serialize_response<V>(request_id: u64, kind: &str, value: V) -> Packet
    where V: Serialize,
{
    let mut buf = Vec::new();
    (RESPONSE, kind, request_id, &value)
        .serialize(&mut Cbor::new(&mut buf))
        .expect("Can always serialize response data");
    return Packet::Binary(buf);
}
