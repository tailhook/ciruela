Sync Command
============

Basic command-line of sync looks like [1]_:

.. code-block:: console

   $ ciruela sync --append=local-dir:/remote/dir cluster.example.org

This means:

1. Connect to cluster ``cluster.example.org``
2. Upload a local directory ``local-dir`` to a virtual remote directory
   ``/remote/dir`` on whole cluster (whatever machines accept this dir)

**Multiple directories** can be added simultaneously as well as **multiple
clusters**.

There are couple of upload modes:

* ``--append`` -- add directory if not exists, fails if the directory exists
  *and* its hash doesn't match currently uploaded one
* ``--append-weak`` -- add directory if not exists, but ignore if it exists
* ``--replace`` -- replace a directory on the remote system(s), this only
  works if directory configured with :ref:`append-only <append-only>`
  of ``false``

Each cluster specified is processed by the same algorithm, which is basically:

1. Find three nodes
2. Subscribe for notifications
3. Start upload
4. Wait for all notifications to complete

More details in :ref:`sync-flow`. All of them are processed at the same time.

If you don't have a common hostname for your cluster you may use ``-m``
instead:

.. code-block:: console

   $ ciruela sync --append=local-dir:/remote/dir \
     -m s1.example.org s2.example.org s3.example.org

This works the same but tedious to write and hard to maintain.

See ``ciruela --help`` for more options.

.. [1] You also need keys for upload. See :ref:`client-keys`
