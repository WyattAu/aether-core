package aether

import timepkg "time"

var (
	Version   = "0.2.0"
	GitCommit = "unknown"
	BuildDate = "unknown"
)

func GetVersion() string {
	return Version
}

var Now = func() Time {
	return Time{Time: timeNow()}
}

func timeNow() timepkg.Time {
	return timepkg.Now()
}

type Time struct {
	timepkg.Time
}
