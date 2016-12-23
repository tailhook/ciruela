===================
Configuring Ciruela
===================


Daemon Configuration
====================

All configs are in base directory, by default it's ``/etc/ciruela``:

* ``master.key`` -- a plain-text list of master keys for this server. Master
  key is that might be used to update configs or upload a key for a new
  user/project. It may be empty if you have another means to deliver these
  configs/keys. Format is similar to ``authorized_keys`` (just keys,
  no parameters)
* ``keys/*.key`` -- key files that might be used in configs, any key file
  may contain multiple keys and any of them might be used when this name
  is specified in config
* ``configs/*.yaml`` -- a list of configs for syncing images
* ``overrides.yaml`` -- contains overrides of things in ``configs`` for this
  specific machine

If ``master.key`` exists you can't edit ``keys`` and ``configs`` locally,
they will be reset or deleted (because of checksum mismatch) on next service
restart or earlier. You can edit overrides, however.

Thigs in ``configs/*.yaml`` are all equal. You can keep everything in single
file or have multiple ones at your convenience.
