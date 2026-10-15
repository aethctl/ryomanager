package main

import (
	"os"
	"os/exec"
	"strings"
	"syscall"
	"testing"
	"time"
)

func spawn(t *testing.T) (int, uint64) {
	t.Helper()
	cmd := exec.Command("sleep", "300")
	if err := cmd.Start(); err != nil {
		t.Fatalf("start a process to end: %v", err)
	}
	pid := cmd.Process.Pid
	t.Cleanup(func() {
		_ = cmd.Process.Kill()
		_, _ = cmd.Process.Wait()
	})
	start, ok := processStartTime(pid)
	if !ok {
		t.Fatalf("no start time for pid %d, which the identity check needs", pid)
	}
	go func() { _, _ = cmd.Process.Wait() }()
	return pid, start
}

func gone(t *testing.T, pid int) bool {
	t.Helper()
	for i := 0; i < 100; i++ {
		if err := syscall.Kill(pid, 0); err != nil {
			return true
		}
		time.Sleep(10 * time.Millisecond)
	}
	return false
}

func appWithGroup(t *testing.T, key string, members [][2]uint64) *App {
	t.Helper()
	a := NewApp()
	a.collector.mu.Lock()
	a.collector.groups[key] = members
	a.collector.mu.Unlock()
	return a
}

func TestEndGroupOutlivesAMemberThatAlreadyExited(t *testing.T) {
	alivePID, aliveStart := spawn(t)
	deadPID, deadStart := spawn(t)
	_ = syscall.Kill(deadPID, syscall.SIGKILL)
	if !gone(t, deadPID) {
		t.Fatalf("could not get pid %d out of the way", deadPID)
	}

	a := appWithGroup(t, "g", [][2]uint64{
		{uint64(deadPID), deadStart},
		{uint64(alivePID), aliveStart},
	})
	if err := a.EndGroup("g", true); err != nil {
		t.Fatalf("ending the group reported %v; the one that exited is not a failure", err)
	}
	if !gone(t, alivePID) {
		t.Fatal("the member that was still running was never signalled")
	}
}

func TestEndGroupSaysWhenThereWasNothingLeft(t *testing.T) {
	pid, start := spawn(t)
	_ = syscall.Kill(pid, syscall.SIGKILL)
	if !gone(t, pid) {
		t.Fatalf("could not get pid %d out of the way", pid)
	}

	a := appWithGroup(t, "g", [][2]uint64{{uint64(pid), start}})
	err := a.EndGroup("g", true)
	if err == nil {
		t.Fatal("reported success for a group with nothing left in it")
	}
	if !strings.Contains(err.Error(), "exited") {
		t.Errorf("said %q, which does not tell a person the group was already gone", err)
	}
}

func TestIdentityRefusesAReusedPid(t *testing.T) {
	pid, start := spawn(t)
	if err := validateIdentity(pid, start); err != nil {
		t.Fatalf("refused a live process with its own start time: %v", err)
	}
	if err := validateIdentity(pid, start+1); err == nil {
		t.Error("accepted a start time that is not this process's; a reused pid would be signalled")
	}
	if err := validateIdentity(1, start); err == nil {
		t.Error("accepted pid 1")
	}
	if err := validateIdentity(os.Getpid(), start); err == nil {
		t.Error("accepted its own pid")
	}
}

func TestEndGroupRefusesAKeyItDoesNotHold(t *testing.T) {
	a := NewApp()
	if err := a.EndGroup("nothing-by-that-name", false); err == nil {
		t.Fatal("accepted a group key the collector never saw")
	}
}

func TestEndGroupWillNotSignalAPidThatWasHandedOn(t *testing.T) {
	pid, start := spawn(t)
	a := appWithGroup(t, "g", [][2]uint64{{uint64(pid), start + 1}})
	if err := a.EndGroup("g", true); err == nil {
		t.Fatal("reported success for a group whose only member has a different identity")
	}
	if gone(t, pid) {
		t.Fatal("signalled a pid whose recorded start time does not match the process holding it")
	}
}

func TestEndGroupSaysWhyItWouldNotTouchAProtectedProcess(t *testing.T) {
	a := appWithGroup(t, "g", [][2]uint64{{1, 0}})
	err := a.EndGroup("g", false)
	if err == nil {
		t.Fatal("reported success for a group it refused to signal")
	}
	if strings.Contains(err.Error(), "exited") {
		t.Errorf("said %q about a process that is running", err)
	}
	if !strings.Contains(err.Error(), "protected") {
		t.Errorf("said %q, which does not say why nothing happened", err)
	}
}

func TestManyRefusalsStayOneReadableLine(t *testing.T) {
	members := make([][2]uint64, 0, 300)
	for i := 0; i < 300; i++ {
		members = append(members, [2]uint64{1, 0})
	}
	a := appWithGroup(t, "g", members)
	err := a.EndGroup("g", false)
	if err == nil {
		t.Fatal("reported success for a group it refused to signal")
	}
	if len(err.Error()) > 200 {
		t.Errorf("the message is %d bytes; the strip that shows it has no scrollbar", len(err.Error()))
	}
	if !strings.Contains(err.Error(), "300") {
		t.Errorf("said %q, which does not say how many were refused", err)
	}
}

func TestTheCountIsOfWhatWasStillThere(t *testing.T) {
	alivePID, aliveStart := spawn(t)
	deadPID, deadStart := spawn(t)
	_ = syscall.Kill(deadPID, syscall.SIGKILL)
	if !gone(t, deadPID) {
		t.Fatalf("could not get pid %d out of the way", deadPID)
	}

	a := appWithGroup(t, "g", [][2]uint64{
		{uint64(deadPID), deadStart},
		{uint64(alivePID), aliveStart},
		{1, 0},
	})
	err := a.EndGroup("g", true)
	if err == nil {
		t.Fatal("said nothing about the member it refused")
	}
	if !strings.Contains(err.Error(), "ended 1 of 2") {
		t.Errorf("said %q; the one that had already exited is not something that was tried", err)
	}
}
