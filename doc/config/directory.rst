.. _directory-config:

================
Directory Config
================


A config in ``/etc/ciruela/configs/NAME.yaml`` describe synchronization of a
single directory.

See :ref:`overview of the configuration <config>` and `config syntax`_
for basics.

.. _config syntax: http://rust-quire.readthedocs.io/en/latest/


Example
=======

This is somewhat minimal config:

.. code-block:: yaml

   directory: /var/containers
   append-only: true
   num-levels: 1
   upload-keys: &keys [user1, ci2]
   download-keys: *keys

All propererties here are required except ``*-keys``. We may make more
settings optional later, when more patterns appear. Note here we reuse same
list of keys both for ``upload-keys`` and ``download-keys`` by using YAML
alias and anchor.


.. index:: pair: directory; Directory Config
.. describe:: directory

   (required) A base path to a directory where these paths will be placed.
   This directory can be (temporarily) overridden in ``overrides.yaml``.

.. index:: pair: append-only; Directory Config
.. describe:: append-only

   (required) If set to ``true`` uploads will be rejected if a subdirectory
   with same name but different contents will be uploaded. It's considered
   good design to use ``append-only: true`` if possible.

   It only makes sense if ``num-levels`` is non-zero.

   .. note:: Under some circumstances the contents of the uploaded directory
      can be changed as a part of reconciliation of the cluster (i.e. if
      different hosts accepted different contents for the directory).

      So if you have strict requirements you have to use some consistent
      storage to bookkeep contents (ciruela is AP system, meaning it prefers
      availability over consistency).

.. index:: pair: num-levels; Directory Config
.. describe:: num-levels
.. _num-levels:

   (required) Number of levels of subdirectories to accept. Zero means no
   subdirectory, meaning the directory has to be atomically uploaded as
   a whole. Zero is useless with ``append-only: true``. Otherwise arbitrary
   positive integer may be specified although some small value like 1, 2 or
   maybe 3 make the most sense.

   Let's study some use cases:

   1. ``/var/containers`` contain directory for containers. Each container
      is ``/etc/containers/app.v123``. Set ``num-levels`` to ``1`` and
      ``append-only`` to ``true``.
   2. ``/etc/nginx`` contain nginx configuration. Set ``num-levels`` to ``0``,
      and ``append-only`` to ``false``. In this case you will always upload
      the whole nginx config and it will switch atomically.
   3. ``/var/indices`` contains multiple indexes of some imaginary replicated
      DB and each index has multiple versions:
      ``/var/indices/documents/20170101-1653``. Set ``num-levels`` to ``2``
      and ciruela will automatically create first level directories and will
      atomically update and move second-level directories.

   .. note:: When ``num-levels`` is ``0`` ciruela must be able to write a
      to the parent directory of the ``directory``. For example, if you
      want to update ``/etc/ningx``, the tool is going to write
      ``/etc/.tmp.nginx.cr1d2e3a`` then atomically move it to ``/etc/nginx``.


.. index:: pair: upload-keys; Directory Config
.. index:: pair: download-keys; Directory Config
.. describe:: upload-keys, download-keys

   Keys that are authorized to upload and download contents of the directory.

   Each name of the key corresponds to name of the keyfile in
   ``/etc/ciruela/keys/NAME.key``, multiple keys can be listed in that file.

   Master key is always allowed to upload and download contents. So if no
   ``upload-keys`` specified the master key is only way to upload files there.
