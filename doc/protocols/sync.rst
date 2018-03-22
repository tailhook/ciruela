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

On the failure path of (1) server returns a list of hosts where to connect
to. Client establishes new connections and repeats a cycle of
:ref:`PublishImage` and :ref:`AppendDir` to few of the specified hosts so that
number of connections to hosts which accepted directory are three.

.. [1] In rust API the number can be configured__. In future, we might add
   a command-line parameter too.

__ https://docs.rs/ciruela/0.5.12/ciruela/cluster/struct.Config.html#method.initial_connections

(TBD)
