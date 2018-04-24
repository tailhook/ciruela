.. _use-cases:

=========
Use Cases
=========

This section describles various setups for ciruela for solving specific kinds
of problems. You can mix multiple things in single ciruela server, the only
current limination is: all the files and directories created by ciruela have
single owner (because we don't want to run daemon as root).

In this examples we use all the defaults:

1. User running ``ciruela-server`` is ``ciruela``
2. Configuration dir is ``/etc/ciruela``
3. Ciruela port is default ``24783``


.. toctree::
   :maxdepth: 2
   :caption: Contents:

   syncing-configs
