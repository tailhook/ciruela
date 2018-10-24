==============
Remote Editing
==============


Setup
=====

Initial setup is the same as for :ref:`syncing-configs`.

The most important thing is this one:

.. code-block:: yaml
    :emphasize-lines: 3

    directory: /etc/daemonio-configs
    num-levels: 1
    append-only: false
    upload-keys: [my_daemonio]


Client setup is also the same. If you can ``sync --replace`` you can also
edit some file:


How it Works
============

.. code-block:: console

   ciruela edit server.name --dir /my-daemonio/current --file /config.yaml

This will do the following:

1. Download a specified file from a specified directory
2. Launch whatever is specified in ``CIRUELA_EDITOR``, ``VISUAL`` or
   ``EDITOR`` environment variables (in that order) or ``vim`` if none.
2. If editor is exited successfully upload the new file back to the original
   directory


