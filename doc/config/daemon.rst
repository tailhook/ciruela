====================
Daemon Configuration
====================

Default configuration directory is ``/etc/ciruela`` it's structure looks
like this:


.. code-block:: bash

    /etc/ciruela
    ├── master.key   # optional
    ├── peers.txt    # optional
    ├── configs
    │   ├── dir1.yaml
    │   └── dir2.yaml
    └── keys
        ├── key-of-admins.key
        ├── key-of-gitlab.key
        ├── project1.key
        └── project2.key

More specifically:

``master.key``
    a plain-text list of master keys for this server. Master key is that might
    be used to upload data to any directory. On production deploymejnts
    master keys are rarely used. Format is similar to ``authorized_keys``
    of SSH daemon (just keys no parameters): one line per key, arbitrary
    comment a the end.

``keys/*.key``
    key files that might be used in configs, any key file may contain multiple
    keys (similarly to ``master.key`` or ``authorized_keys``) and any of them
    might be used when this name is specified in directory config

``configs/*.yaml``
    a config per directory. I.e. if there is ``dir1.yaml``, this means you can
    upload to ``/dir1/something...``

``peers.txt``
    plain list of IP addresses and hostnames to distribute files too, only
    used/needed if ``--cantal`` command-line option is not specified.


.. note:: All configs are reloaded only on restart of the server. Restarting
   should be seamless if doesn't happen to often (if there is upload in
   progress, client should reconnect and continue gracefully).
