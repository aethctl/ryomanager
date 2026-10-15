package main

import (
	"fmt"
	"os"
	"os/exec"
	"testing"
	"time"
)

func TestAProcessStartedJustNowIsDatedJustNow(t *testing.T) {
	if bootTime() == 0 {
		t.Skip("/proc/stat does not say when this machine booted")
	}
	before := uint64(time.Now().Unix())
	cmd := exec.Command("sleep", "5")
	if err := cmd.Start(); err != nil {
		t.Fatalf("start a process to date: %v", err)
	}
	t.Cleanup(func() {
		_ = cmd.Process.Kill()
		_, _ = cmd.Process.Wait()
	})
	after := uint64(time.Now().Unix())

	raw, err := os.ReadFile(fmt.Sprintf("/proc/%d/stat", cmd.Process.Pid))
	if err != nil {
		t.Fatalf("read the stat line of the process just started: %v", err)
	}
	p, _, ok := parseProcess(cmd.Process.Pid, string(raw))
	if !ok {
		t.Fatal("could not read the stat line of the process just started")
	}
	if p.StartedAt+1 < before || p.StartedAt > after+1 {
		t.Errorf("a process started between %v and %v is dated %v",
			time.Unix(int64(before), 0), time.Unix(int64(after), 0), time.Unix(int64(p.StartedAt), 0))
	}
	if p.StartTime == 0 || p.StartTime == p.StartedAt {
		t.Errorf("the identity token must stay the tick count, got %d beside %d", p.StartTime, p.StartedAt)
	}
}

func TestTickZeroIsTheMomentTheMachineBooted(t *testing.T) {
	boot := bootTime()
	if boot == 0 {
		t.Skip("/proc/stat does not say when this machine booted")
	}
	if got := startedAt(0); got != boot {
		t.Errorf("a process started at the first tick started when the machine did: %d, wanted %d", got, boot)
	}
}

func TestThisProcessDidNotStartInNineteenSeventy(t *testing.T) {
	raw, err := os.ReadFile("/proc/self/stat")
	if err != nil {
		t.Skip("no /proc on this machine")
	}
	p, _, ok := parseProcess(os.Getpid(), string(raw))
	if !ok {
		t.Fatal("this process could not read its own stat line")
	}
	now := uint64(time.Now().Unix())
	if p.StartedAt < now-3600 || p.StartedAt > now+5 {
		t.Errorf("this test started at %v, which is not now", time.Unix(int64(p.StartedAt), 0))
	}
	if p.StartTime == 0 || p.StartTime == p.StartedAt {
		t.Errorf("the identity token must stay the tick count, got %d beside %d", p.StartTime, p.StartedAt)
	}
}
