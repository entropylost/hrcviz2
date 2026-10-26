#!/usr/bin/env nu
# Builds the site and pushes dist/ to the gh-pages branch.
# The gh-pages branch is checked out as a worktree in .gh-pages/, since trunk wipes dist/ on every build.

def succeeds [cmd: closure]: nothing -> bool {
	(do $cmd | complete).exit_code == 0
}

def main [--no-push] {
	cd $env.FILE_PWD

	let pages = ".gh-pages"
	let branch = "gh-pages"

	if not ($pages | path join ".git" | path exists) {
		git worktree prune
		if (succeeds { git show-ref --verify --quiet $"refs/heads/($branch)" }) {
			git worktree add $pages $branch
		} else if (succeeds { git fetch origin $branch }) {
			git worktree add -b $branch $pages $"origin/($branch)"
		} else {
			git worktree add --orphan -b $branch $pages
		}
	}

	trunk build --release

	mut rev = git rev-parse HEAD | str trim
	if (git status --porcelain | is-not-empty) {
		$rev = $"($rev)-dirty"
	}

	rsync -a --delete --exclude .git dist/ $"($pages)/"
	touch ($pages | path join ".nojekyll")

	git -C $pages add -A
	if (succeeds { git -C $pages diff --cached --quiet }) {
		print "No changes to deploy."
	} else {
		git -C $pages commit -q -m $"Deploy ($rev)"
		git -C $pages log -1 --oneline
	}

	if not $no_push {
		git -C $pages push -u origin $branch
	}
}
