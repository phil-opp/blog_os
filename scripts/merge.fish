set original (git rev-parse --abbrev-ref HEAD)
for x in (seq 99)
    set previous (printf "post-%02i" $x)
    set current (printf "post-%02i" (math $x + 1))
	if not git checkout $current --quiet
        break
    end
    if not git merge $previous --no-edit
        break
    end
end
git checkout $original --quiet
