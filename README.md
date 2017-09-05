Ciruela
=======

|              |                                           |
|--------------|-------------------------------------------|
|Documentation | http://tailhook.github.io/ciruela/        |
|Status        | beta [¹](#1)                              |


A peer-to-peer synchronization software for servers in datacenters.


Look and Feel
=============

Upload a folder from your local machine to a cluster::

    > ciruela upload -d myapp mycluster.example.com:/apps/myapp/v1.1.2
    Indexing...
    Done. Indexed 1333 dirs, 11306 files, 2517 symlinks.
    Connected to mycluster.example.com. It has 127 peers.
    Sending index...
    Signature is okay. Virtual dir /apps/myapp accepted by 78 peers.
    Uploading...
    Uploaded 143 files (90% blocks reused), to 3 peers.
    It's safe to disconnect now. Switching to monitor mode...
    Done. Synced to 78 peers.


License
=======

Licensed under either of

* Apache License, Version 2.0,
  (./LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license (./LICENSE-MIT or http://opensource.org/licenses/MIT)
  at your option.

Contribution
------------

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.

