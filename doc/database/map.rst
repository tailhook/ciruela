===
Map
===

``/var/lib/ciruela/map``

Map contains a list of hosts that we know to contain specific images or files
and blocks.

For now ciruela maintains the map of all images that are contained in
``indexes`` directory. But this may be changed in future.

The map is stored in two instances:

1. ``map/full/<hash:0..2>/<hash>.json``
2. ``map/partial/<hash:0..2>/<hash>.cbor``

The path scheme is same as for :ref:`Indexes <db-indexes>` (i.e. image hash
with first two hex characters being a directory).

The ``full`` part contains just a list of hosts that contain full
representation of the image.

The ``partial`` directory contains the binary map of which blocks exist on
each host. The ``partial`` thing doesn't guaranteed to be consistent and
propagated to every host. It's there only to assist initial download of the
image to this host and may be removed as quick as host has received the full
image.

.. note:: We don't store list of blocks that has already been downloaded while
   download is in progress on disk. If ciruela crashes in the middle of the
   download it scans whole on-disk data to find out which blocks have already
   been downloaded and requests others from peers.
