# The two-word interface. Ten recipes, the same words in every tree here.
#
# This file defines no recipe except `default`, and it must stay that way.
# just 1.58.0 lets an importing file beat what it imports, so any recipe
# written here would permanently shadow the language layer's real one. Among
# imports the first listed wins, which is why the order below runs most
# specific to least.
#
# Without `allow-duplicate-recipes` there is no override at all. Under just
# 1.58.0 a base and a language layer both defining `test` is a hard error that
# kills every recipe in the tree:
#
#     $ just --list
#     error: recipe `test` first defined on line 3 is redefined on line 15
#
# `just test`, `just checks` and every other recipe fail the same way. The
# setting turns that error into first-listed-wins, so removing it breaks the
# whole tree.
#
#     just/project.just   this project's own. No template writes it.
#     just/lang.just      the language layer's.
#     just/base.just      the ten, supplied for every project.
#
# `default` is the exception and has to be here: defined in an import it is not
# found, and bare `just` answers "justfile contains no default recipe".
#
# Every project answering to the same two words is a settled decision.
set allow-duplicate-recipes := true

import? 'just/project.just'
import? 'just/lang.just'
import  'just/base.just'

default:
    @just --list
