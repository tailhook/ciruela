===================
Websockets Protocol
===================


We use standard websockets handshake with
``Sec-WebSocket-Protocol: ciruela.v1`` and no extensions.


Serialization
-------------

Payload is serialized using CBOR_. There are three kinds of messages:

1. Request
2. Response
3. Notification

All three types of messages can be sent at any time into any direction. Each
request includes a numeric identifier that is used in corresponding response.
Each side of the connection can create request identifiers independently.
Each request has exactly one response. If more than one response is provided
it's built by some higher level construct.

Every message is contiguous, messages can't interleaved. Protocol has no
flow control besides what TCP provides. If more concurrency desired than
multiple connections might be used.

We will use CDDL_ for describing message format. Here is the basic
structure of a message:

.. code-block:: cddl

   message = $message .within message-structure

   message-structure = [message-kind, message-type, *any] .and typed-message
   message-kind = &( notification: 0, request: 1, response: 2 )
   message-type = $notification-type / $request-type

   typed-message = notification / request / response
   notification = [0, $notification-type, *any]
   request = [1, $request-type, request-id, *any]
   response = [2, $request-type, request-id, *any]
   request-id = uint

.. _signing-uploads:

Signing Uploads
---------------

Signature of the upload consists of the following fields packed as the
CBOR length-prefixed array in this specific order:

.. code-block:: cddl

    signature-data = [
        path: text,      ; destination path
        image: bytes,    ; binary hashsum of the image (bottom line of the
                         ; index file but in binary form)
        timestamp: uint, ; milliseconds since unix epoch when image was signed
    ]

Ciruela currently only supports ed25519 algorithm for signatures, but more
alorithms (RSA in particular) can be used in future.

The ``signature`` itself is an array of at least two arguments with type as
the first element and rest depends on the signature algorithm:

.. code-block:: cddl

   signature = ["ssh-ed25519", bytes .size 64]

Note: the ed25519 signature includes public key as a part of the signature as
per standard. Other signatures might require different structure.


Commands
--------


AppendDir
`````````

Schedule a an adding the new directory. This sends only a signed hash of the
directory index and marks this directory as incoming.

.. note:: If different images have been scheduled for upload by different
   peers in the cluster cluster may end up with different images on different
   nodes

If upload for this path and image already exists at node another signature
is added.

If there is no such index on the peer it asks this peer or any other available
connection for the index data itself and subsequently asks for missing chunks
(some chunks may be reused from different image).

Content of the message is a dictionary (CBOR object):

.. code-block:: cddl

    $message /= [1, "AppendDir", request-id, append-dir-params]
    $message /= [2, "AppendDir", request-id, append-dir-response]
    append-dir-params = {
        path: text,                 ; path to put image to
        image: bytes,               ; binary hashsum of the image (bottom line
                                    ; of the index file but in binary form
        timestamp: uint,            ; milliseconds since the epoch
        signatures: [+ signature],  ; one or more signatures
    }
    append-dir-response = {
        accepted: bool,             ; whether directory accepted or not
    }

Note: *accepted* response here doesn't mean that this is new directory (i.e.
same directory might already be in place or might still be downloaded). Also
it doesn't mean that download is already complete. Most probably it isn't,
and you should wait for a completion notification.


ReplaceDir
``````````

Schedule a replacing the directory with the new image. This sends only a
signed hash of the directory index and marks this directory as incoming.

.. note:: If different images have been scheduled for upload by different
   peers in the cluster the one with latest accross the cluster timestamp
   in the signature will win

If there is no such index on the peer it asks this peer or any other available
connection for the index data itself and subsequently asks for missing chunks
(some chunks may be reused from different image).

.. code-block:: cddl

    $message /= [1, "ReplaceDir", request-id, replace-dir-params]
    $message /= [2, "ReplaceDir", request-id, replace-dir-response]
    replace-dir-params = {
        path: text,                 ; path to put image to
        image: bytes,               ; binary hashsum of the image (bottom line
                                    ; of the index file but in binary form)
        ? old_image: bytes,         ; hash olf the previous image
        timestamp: uint,            ; milliseconds since the epoch
        signatures: [+ signature],  ; one or more signatures
    }
    replace-dir-response = {
        ; TODO(tailhook) figure out
    }

Note: if no ``old_image`` is specified the destination directory is not
checked. Use ``AppendDir`` to atomically update first image.


PublishImage
````````````

Notifies peer that this host has data for the specified index. This is usually
executed before ``AppendDir``, so that when receiving latter command server
is already aware where to fetch data from.

.. code-block:: cddl

    $message /= [0, "PublishImage", publish-index-params]
    publish-image-params = {
        id: bytes,               ; binary hashsum of the image (bottom line
                                 ; of the index file but in binary form)
    }


This notification basically means that peer can issue ``GetIndex`` in
backwards direction.


ReceivedImage
`````````````

Notifies peer that some host (maybe this one, or other peer) received
and commited this image. The notification is usually sent after
``PublishImage`` for the specified id.

The notification can be used by cicuela command-line client to determine that
at least one host (or at least N hosts) received the image and it's safe to
disconnect from the network and also to display progress.

.. code-block:: cddl

    $message /= [0, "ReceivedImage", recevied-image-params]
    received-image-params = {
        id: bytes,               ; binary hashsum of the image (bottom line
                                 ; of the index file but in binary form)
        path: text,              ; path where image was stored
        hostname: string,        ; hostname of the receiver
        forwarded: bool,         ; whether message originated from this host
                                 ; or forwarded
    }

The ``forwarded`` field might be used to skip check on ``hostname`` field.


GetIndex
````````

Fetch an index data by it's hash. This method is usually called by server
after `AppendDir` and `ReplaceDir` has been received. And it is sent to
the original client (in backwards direction). But the call only takes place
if no index already exists on this host or on one of the peers.

.. code-block:: cddl

    $message /= [1, "GetIndex", request-id, get-index-params]
    $message /= [2, "GetIndex", request-id, get-index-response]
    get-index-params = {
        id: bytes,               ; binary hashsum of the image (bottom line
                                 ; of the index file but in binary form)
    }
    get-index-response = {
        ? data: bytes,           ; full original index file
    }

Note: index file can potentially be in different formats, but in any case:

* Consistency of index file is verified by original `id` which is also a
  checksum
* Kind of index can be detected by inspecting data itself (i.e. first bytes of
  index file should contain a signature of some kind)


GetBlock
````````

Fetch a block with specified hash.

.. code-block:: cddl

    $message /= [1, "GetBlock", request-id, get-block-params]
    $message /= [2, "GetBlock", request-id, get-block-response]
    get-block-params = {
        hash: bytes,             ; binary hashsum of the block
    }
    get-block-response = {
        ? data: bytes,           ; full original index file
    }

.. _cbor: http://cbor.io/
.. _cddl: https://tools.ietf.org/html/draft-greevenbosch-appsawg-cbor-cddl-09
