=======
Ciruela
=======


A peer-to-peer synchronization software for servers in datacenters.


Look and Feel
=============

Upload a folder from your local machine to a cluster:

    > ciruela -d myapp mycluster.example.com:/apps/myapp/v1.1.2
    Indexing...
    Done. Indexed 1333 dirs, 11306 files, 2517 symlinks.
    Connected to mycluster.example.com. It has 127 peers.
    Sending index...
    Signature is okay. Virtual dir /apps/myapp accepted by 78 peers.
    Uploading...
    Uploaded 143 files (90% blocks reused), to 3 peers.
    It's safe to disconnect now. Switching to monitor mode...
    Done. Synced to 78 peers.

