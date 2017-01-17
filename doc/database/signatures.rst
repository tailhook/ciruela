==========
Signatures
==========

Signatures are stored in ``/var/lib/ciruela/signatures``. The structure of
this directory somewhat replicates the structure of destination directories.

I.e. if you have a :ref:`directory-config` ``/etc/ciruela/configs/images.yaml``,
which configured as :ref:`num-levels: 1<num-levels>`, and you have uploaded an
image ``hello.123``, you will have the following files:

* ``/var/lib/ciruela/signatures/images/hello.123.log``
* ``/var/lib/ciruela/signatures/images/hello.123.state``

First file contains just a log of signatures as they were uploaded or fetched
from other hosts. The second file contains state of the destination directory.
