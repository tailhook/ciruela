.. _db-indexes:

=======
Indexes
=======


Index is basically a list of files and directories and their checksums that
will be reconstructed when image is downloaded. The indexes are stored in
``/var/lib/ciruela/indexes``.

Supported index formats:

* ``.ds1`` -- `dir-signature v1`_

Indexes are named by it's own hash (not the hash of the index file itself but
the hash that is stored in the bottom line of the index file). The first
two characters of a hash file are the directory name, so the full path of
the index file is:

    /var/lib/ciruela/indexes/e8/e8082d95318cb704297975988aca7b95770a3d6bb3023687dae68dcfff644d84.ds1

Note: we store all indexes here regardless of which user requested the upload
and what directory it should be put into. Retention policy of index files is
very much different to the retention of the files themselves. In the first
implementation we keep all indexes that are used anywhere in the cluster on
every node. We're considering to tighten the scope in future.

.. _dir-signature v1: https://github.com/tailhook/dir-signature/blob/master/FORMAT.v1.rst

