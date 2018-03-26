How Sync Works
==============

This documents tries to describe what happens after you run:

.. code-block:: console

   $ ciruela sync --append=local-dir:/remote/dir cluster.example.org

This might look easy: just upload all the files to all the machines, but it's
not so simple. Here is a non-comprehensive list of complexities:

1. We don't want to upload to all machines, because it's inefficient
2. But we want status of all uploads on all servers to be delivered
   to the client running ``sync`` anyway
3. Not all directories are accepted on all nodes, we want single entrypoint
   hostname for all of them anyway
4. Connections might be interrupted and some nodes are down
5. Download acks can be lost (see point above) and dir might already be there
   even when starting sync
6. Nodes don't have persistent connections between each other too,
   for efficiency


(0) Indexing
------------

Some offline preparation is done: scan specified directory and make an "index"
of it.  Index is a file that contains list of paths and hashsums of the file
contents.


(1) Direct Connections
----------------------

First ciruela resolves ``cluster.example.org`` and chooses a random sample
of up to **three** [1] individual IP addresses to connect to.

On each direct connection client firstly sends :ref:`PublishImage` then either
:ref:`AppendDir` or :ref:`ReplaceDir` request. Upon receiving request ciruela
(daemon) does the following things:

1. Checks if this directory is configured on this server
2. Checks whether path exists and it's id matches request,
   if both are false rejects AppendDir command
3. Checks whether signature matches any of accepted keys for that directory
4. Registers that this image should be downloaded to this path

On the failure path of (1) server returns a list of hosts where to connect
to. Client establishes new connections and repeats a cycle of
:ref:`PublishImage` and :ref:`AppendDir` to few of the specified hosts so that
number of connections to hosts which accepted directory are three.

The :ref:`PublishImage` call does two things:

1. Registers client as a "source" of the image, so that server knows where to
   fetch this image from [2]_
2. Also server marks that this client is "watching" the download progress for
   this image (so that completion notifications are delivered here later)

.. [1] In rust API the number can be configured__. In future, we might add
   a command-line parameter too
.. [2] We don't send actual image in AppendDir/ReplaceDir call because it's
   expected that either image's index or some blocks of the actual data can
   exist on the destination host

__ https://docs.rs/ciruela/0.5.12/ciruela/cluster/struct.Config.html#method.initial_connections


(2) Download Process
--------------------

The initial :ref:`AppendDir` / :ref:`ReplaceDir` kicks off the whole cluster
synchronization process.

1. Right after registration initial node sends "download progress" message
   to few random nodes (with 0 progress at this point) [1]_
2. Then ciruela computes hash of the parent directory of the uploaded path
   and sends that hash to few random nodes [1]_
3. Each node (including first ones) starts the download from fetching index
   (if not already cached here)
4. Then server looks in several folder in the same dir of whether there are
   files which are exactly like ones being downloaded, if there are it
   hardlinks all such files in the new directory.
5. Then it starts to download blocks (the actual file data)

Each index and block download works approximately by the following agorithm:

1. TBD

.. [1] These two messages serve different purpose. The "download progress"
   message is to find out where blocks of the image are already avaialable,
   so we can fetch them from that host. And hash of the parent directory is
   used to initiate downloads.


(TBD)
