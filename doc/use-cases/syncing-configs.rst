===============
Syncing Configs
===============

Let's say we want to configure a daemon called ``demonio`` (which is spanish
for daemon).

About ``daemonio``:

1. It stores it's configs in the directory ``/etc/daemonio.d``
2. It can detect configuration changes itself (See :ref:`Reloading Config`)
3. This config of daemon is named ``my_daemonio`` [#instances]_


.. [#instances] If we ever run multiple instances of the daemon on the same
   machine or accross the same cluster we need to differentiate different
   config folders


Server Setup
============

1. Configure the service to read configs from ``/etc/daemonio-configs/current`` instead of ``/etc/daemonio.d``


   .. note:: The point here is: ciruela can sync whole directory of configs and
      replace it atomically. But to do that we need to grant access for user
      to the parent directory of the actual config folder. We don't want to
      grant access to the whole ``/etc``.

2. Drop the following file into ``/etc/ciruela/configs/my-daemonio.yaml``:

    .. code-block:: yaml

        directory: /etc/daemonio-configs
        num-levels: 1
        append-only: false
        upload-keys: [my_daemonio]

3. Drop the **public key** allowed to upload new config into
   ``/etc/ciruela/keys/my_daemonio.key``. (See `Client Setup`_)

   .. note:: Multiple keys can be put into the file as well as multiple files
      can be specified in ``upload-keys`` in the configuration of the
      directory. It's useful to create a file per actual user (potentially
      having multiple keys) or organize keys in any way you want.

4. Set ``ciruela`` as the owner of all the directories it should sync::

        chown -R ciruela /etc/daemonio-configs


Client Setup
============


Human Operator Setup
--------------------

This is mostly useful for testing, because work well mostly for a single user.

1. Generate a key, it's same as ssh but in ``ed25519`` format::

        ssh-keygen -t ed25519 -f ~/.ssh/id_ciruela

   .. note::

      Ciruela doesn't support password-protected keys yet.

      This also means you can use your normal ``~/.ssh/id_ed25519`` key, if
      it isn't password-protected, but leaving ssh key plain-text isn't good
      idea. Ciruela keys are much more lower risk because they allow only
      uploading a limited set of directories rather than executing any
      script.

      Also you may wish to delete the key and use CI system when you finish
      testing the setup.


2. Upload the key into ``/etc/ciruela/keys/my_daemonio.key``.

3. Run the following every time you need to upload new configs::

       ciruela upload --replace -d local_config_copy server.name:/my-daemonio/current


CI Setup
--------

Usually CI systems allow to put secret variables into environment, so
you can use ``CIRUELA_KEY`` environment variable for storing keys.

1. Generate a key, it's same as ssh but in ``id_ed25519`` format::

        ssh-keygen -t ed25519 -f tmp-key

2. Upload the private key into CI for ``CIRUELA_KEY``, for example for
   travis you may use ``travis encrypt``::

       travis encrypt "CIRUELA_KEY=$(cat tmp-key)" --add

3. Add upload command to the task::

       ciruela upload --replace -d ./cfg server.name:/my-daemonio/current


Reloading Configs
-----------------

All of this works if your service can pick up configuration on the fly without
any kind of signals.

Making signals for configuration reload is out of scope of this article but
here are some ideas:

1. You can set a script that compares directory timestamp and signals service
   if that changes. Ciruela replaces directory atomically so reloading is safe
   at any time (or as quick as the directory is new)
2. You can use some special programs for that (but I'm not sure they
   are suited for production):

   * nodemon_
   * modd_
   * watchdog_

3. Some supervisors like supervisord_ (`API <supervisord-api>`_)
   and systemd_ (`API <systemd-api>`_) have RPC for the task
4. Maybe you have a UI for the service?

.. _nodemon: https://github.com/remy/nodemon
.. _modd: https://github.com/cortesi/modd
.. _watchdog: https://pythonhosted.org/watchdog/
.. _supervisord: http://supervisord.org/
.. _supervisord-api: http://supervisord.org/api.html
.. _systemd: https://www.freedesktop.org/wiki/Software/systemd/
.. _systemd-api: https://www.freedesktop.org/wiki/Software/systemd/dbus/


Just to show you that (1) is not as scareful as it sounds, here is an
example script for nginx:

.. code-block:: bash

   #!/bin/sh

   DIR=/etc/nginx/conf
   CMD="nginx -s reload"

   last_stat="$(stat --format="%Z/%Y/%d:%i" "$DIR" || "<absent>")"
   while sleep 1; do
     new_stat="$(stat --format="%Z/%Y/%d:%i" "$DIR" || "<absent>")"
     if [ "$last_stat" != "$new_stat" ]; then
       $CMD
     endif
   done

.. note:: The script doesn't detect most changes done on individual config
   files, but ciruela always replaces the directory with the new one. And we
   detect it by checking inode number ``"%i"``. Other stat parameters here are
   just for being more cautious.


Additional Options
------------------

* If your service has only one configuration file, you should put it into
  a directory anyway, as ciruela syncs directories. But it's a good idea
  since you can add another include file later or just put a README into
  the dir.

* You may want to check configs before uploading.
  For example run ``daemonio --config=./local_config_dir --check-config``
  on the CI server before upload.

* You can override keys via ``-i``, ``-e`` (see ``ciruela upload --help``)

* You can upload to multiple servers via::

    ciruela upload -d x --replace s1.example.org:/dir s2.example.org:/dir

  Ciruela will ensure that at least one server with that host name (if
  host name resolves to multiple addresses) accepts the upload. This works
  even for multiple clusters.

* If server name resolves to multiple IP addresses, ciruela will try to upload
  to at most three of them (random ones if there are more) and will return
  non-zero exit status if none of them accepts the upload.

* Mutliple instances of ``daemonio`` can be configured with a single upload key
  you may put multiple configurations into the single directory:

  * /etc/daemonio-configs/my_daemonio
  * /etc/daemonio-configs/other_daemonio

* Or you can group all configured services under single folder (if you don't
  need to differentiate permissions for them):

  * /etc/syncing-configs/daemonio
  * /etc/syncing-configs/nginx
  * /etc/syncing-configs/my-other-service

