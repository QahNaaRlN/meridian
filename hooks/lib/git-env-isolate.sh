# Meridian Kernel — strip this process's repository-local Git environment.
#
# Git runs hooks with GIT_DIR, GIT_WORK_TREE, GIT_INDEX_FILE and the rest of
# `git rev-parse --local-env-vars` exported into the environment. Any child that
# then runs `git` against a *different* directory inherits those variables and
# operates on the CALLER'S repository instead of its own working directory. The
# pre-push gates build throwaway synthetic repositories and run `git init/add/
# commit` inside them; without this scrub those synthetic commits land on the
# repository being pushed. That has actually happened — a synthetic test once
# committed straight onto a Kernel branch through an inherited GIT_DIR.
#
# Sourcing this file unsets every repository-local Git variable, so each child
# `git` resolves its repository from its own working directory. Everything else
# in the environment — MERIDIAN_KERNEL, MERIDIAN_INSTANCE, PATH, HOME, … — is
# left untouched.
#
# Source it AFTER reading anything you need from the caller's repository
# (`git rev-parse --show-toplevel`, the ref list on stdin, …): once it runs,
# `git` in the sourcing script no longer points at the caller's repository.
#
# POSIX sh, no bashisms. The variable list comes from Git itself, not a
# hard-coded copy, so a future Git that adds another local variable is covered.
#
# Fail-closed: `git rev-parse --local-env-vars` failing is NOT swallowed into a
# "successful" empty list. The status of the command substitution is checked
# explicitly (via the `if` condition, which also keeps the caller's `set -e`
# from aborting before the check runs); on any non-zero exit this helper prints
# a diagnostic and returns non-zero, so the sourcing hook — running under
# `set -e` — exits before a single gate is started rather than running the
# gates with an unscrubbed environment.

if _meridian_local_git_vars=$(git rev-parse --local-env-vars); then
  for _meridian_local_git_var in $_meridian_local_git_vars; do
    unset "$_meridian_local_git_var"
  done
  unset _meridian_local_git_var _meridian_local_git_vars
else
  _meridian_rc=$?
  echo "git-env-isolate: 'git rev-parse --local-env-vars' exited ${_meridian_rc}; refusing to run pre-push gates without a scrubbed Git environment" >&2
  unset _meridian_local_git_vars _meridian_rc
  return 1 2>/dev/null || exit 1
fi
