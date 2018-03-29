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
   upload-keys: [user1, ci2]

All properties above are required, except ``upload-keys``. We may make more
settings optional later, when more patterns appear.

Or a longer example with auto-clean enabled:

.. code-block:: yaml

    # /etc/ciruela/configs/project1.yaml
    directory: /var/containers/project1
    num-levels: 2
    append-only: true
    auto-clean: true
    keep-min-directories: 2
    keep-max-directories: 100
    keep-recent: 1 day
    keep-list-file: /some/external/system/project1-used-containers.txt


Options Reference
=================

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

.. index:: pair: auto-clean; Directory Config
.. describe:: auto-clean

   (default ``false``) Enable cleanup of this directory. Every directory up
   to ``num_levels-1`` is a separate directory to do cleanup according to
   ``keep-*`` rules.

   Here is an example of a directory with auto-clean configured:

   .. code-block:: yaml

        # /etc/ciruela/configs/project1.yaml
        directory: /var/containers/project1
        num-levels: 2
        append-only: true
        auto-clean: true
        keep-min-directories: 2
        keep-max-directories: 100
        keep-recent: 1 day
        keep-list-file: /some/external/system/project1-used-containers.txt

.. index:: pair: keep-list-file; Directory Config
.. describe:: keep-list-file

    (optional) Read the file for a list of subdirs to keep in this directory.
    It's needed to keep external system(s) in sync with expections.

    The file is a directory name per line. If `num-levels` > 1, then the
    path of a directory (``dir1/dir``) per line should be specified.
    Intermediate directories are ignored in this case (empty intermediate
    directories are cleaned when empty).

    Currently, we use the file to skip cleanup of the subdirectories. But we
    will also download the images in the list if new record appears.

    Only used when `auto-clean` is enabled.

.. index:: pair: keep-min-directories; Directory Config
.. describe:: keep-min-directories

    (default ``2``) Minimum number of recent subdirectories to keep for this
    directory.

    Only used when `auto-clean` is enabled.

.. index:: pair: keep-max-directories; Directory Config
.. describe:: keep-max-directories

    (default ``100``) Maximum number of recent subdirectories to keep for this
    directory.

    Only used when `auto-clean` is enabled.

.. index:: pair: keep-recent; Directory Config
.. describe:: keep-recent

    (default ``2 days``) Keep directories uploaded within this number of
    days. Recent directories can be cleaned if there are more than
    ``keep-max-directories`` of them. And older directories are left only if
    there are less than ``keep-min-directories`` ones which are more recent
    than ``keep-recent`` setting.

    Note: we track recency of the directory not by upload timestamp on this
    specific machine, but by timestamp used in signature which is created
    when upload was first initiated into a cluster.

