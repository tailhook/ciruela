Ciruela Changes by Version
==========================


.. _changelog-0.6.12:

Ciruela 0.6.12
--------------

* Timeout for "index download" changed from 30 - 90 seconds
* Bugfix: if there is a file at destination location of the download, remove
  it (previously failed on ``NotADirectory`` error)


.. _changelog-0.6.11:

Ciruela 0.6.11
--------------

* bugfix: without auto-clean enabled ciruela was considering
  keep-max-directories anyway when reconciling


.. _changelog-0.6.10:

Ciruela 0.6.10
--------------

* bugfix: without auto-clean enabled ciruela was considering
  keep-max-directories anyway when scanning list of dirs at start


.. _changelog-0.6.9:

Ciruela 0.6.9
-------------

* Bugfix: fix check for upload finish when there is only one destination node,
  and cluster is bigger than one


.. _changelog-0.6.8:

Ciruela 0.6.8
-------------

* Feature: command-line prints public keys used for signature to stderr before
  upload. This makes it easier to debug keys mismatch.


.. _changelog-0.6.7:

Ciruela 0.6.7
-------------

* Feature: add ``ciruela put-file`` command that adds/replaces a single file
  in the target directory.
* Feature: add old image identifier support in ``ciruela sync --replace``
  (and rust API) which means we can do (limited version of) atomic updates to
  the directory.
* Feature: ``ciruela edit`` now fails if directory was changed a remote system
  while you were editing a file (same failure applies for ``put-file`` too)
* Bugfix: when all discovered hosts have no config ciruela finishes with
  rejection instead of waiting indefinitely
