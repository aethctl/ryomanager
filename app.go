package main

import (
	"context"
	"errors"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"syscall"
)

type App struct {
	ctx       context.Context
	collector *Collector
}

func NewApp() *App                         { return &App{collector: NewCollector()} }
func (a *App) startup(ctx context.Context) { a.ctx = ctx }
func (a *App) Snapshot() Snapshot          { return a.collector.Snapshot() }
func (a *App) AppIcon(key string) *string  { return a.collector.AppIcon(key) }

var errProtected = errors.New("RyoManager will not terminate this protected process")

func validateIdentity(pid int, start uint64) error {
	if pid <= 1 || pid == os.Getpid() {
		return errProtected
	}
	actual, ok := processStartTime(pid)
	if !ok {
		return errors.New("that process has exited")
	}
	if actual != start {
		return errors.New("that process has exited; the PID now belongs to another")
	}
	return nil
}

func (a *App) ProcessDetail(pid int, startTime uint64) (ProcessInfo, error) {
	if err := validateIdentity(pid, startTime); err != nil {
		return ProcessInfo{}, err
	}
	p, ok := a.collector.Process(pid, startTime)
	if !ok {
		return ProcessInfo{}, errors.New("that process is not in the latest snapshot")
	}
	return processDetail(p), nil
}

func (a *App) EndProcess(pid int, startTime uint64, force bool) error {
	if err := validateIdentity(pid, startTime); err != nil {
		return err
	}
	sig := syscall.SIGTERM
	if force {
		sig = syscall.SIGKILL
	}
	return syscall.Kill(pid, sig)
}

func (a *App) EndGroup(key string, force bool) error {
	members, ok := a.collector.GroupMembers(key)
	if !ok {
		return errors.New("that process group has exited")
	}
	sig := syscall.SIGTERM
	if force {
		sig = syscall.SIGKILL
	}
	ended := 0
	refusals := map[string]int{}
	var reasons []string
	refuse := func(reason string) {
		if _, seen := refusals[reason]; !seen {
			reasons = append(reasons, reason)
		}
		refusals[reason]++
	}
	for _, ref := range members {
		pid := int(ref[0])
		if err := validateIdentity(pid, ref[1]); err != nil {
			if errors.Is(err, errProtected) {
				refuse(err.Error())
			}
			continue
		}
		if err := syscall.Kill(pid, sig); err != nil {
			if !errors.Is(err, syscall.ESRCH) {
				refuse(err.Error())
			}
			continue
		}
		ended++
	}
	refused := 0
	for _, n := range refusals {
		refused += n
	}
	switch {
	case ended == 0 && refused == 0:
		return errors.New("that process group has already exited")
	case ended == 0:
		return fmt.Errorf("nothing in this group could be ended: %s", refusalSummary(reasons, refusals))
	case refused > 0:
		return fmt.Errorf("ended %d of %d, and %s", ended, ended+refused, refusalSummary(reasons, refusals))
	}
	return nil
}

func refusalSummary(reasons []string, counts map[string]int) string {
	parts := make([]string, 0, len(reasons))
	for i, reason := range reasons {
		if i == 3 {
			parts = append(parts, fmt.Sprintf("%d other reasons", len(reasons)-3))
			break
		}
		if n := counts[reason]; n > 1 {
			parts = append(parts, fmt.Sprintf("%d refused: %s", n, reason))
			continue
		}
		parts = append(parts, reason)
	}
	return strings.Join(parts, "; ")
}

func (a *App) SignalProcess(pid int, startTime uint64, action string) error {
	if err := validateIdentity(pid, startTime); err != nil {
		return err
	}
	var sig syscall.Signal
	switch action {
	case "suspend":
		sig = syscall.SIGSTOP
	case "resume":
		sig = syscall.SIGCONT
	default:
		return errors.New("unknown process action")
	}
	return syscall.Kill(pid, sig)
}

func (a *App) SetPriority(pid int, startTime uint64, nice int) error {
	if err := validateIdentity(pid, startTime); err != nil {
		return err
	}
	if err := syscall.Setpriority(syscall.PRIO_PROCESS, pid, nice); err != nil {
		if errors.Is(err, syscall.EACCES) || errors.Is(err, syscall.EPERM) {
			return errors.New("raising priority needs root")
		}
		return err
	}
	return nil
}

func (a *App) OpenLocation(pid int, startTime uint64) error {
	if err := validateIdentity(pid, startTime); err != nil {
		return err
	}
	var path string
	if exe, err := os.Readlink(fmt.Sprintf("/proc/%d/exe", pid)); err == nil {
		path = filepath.Dir(exe)
	}
	if path == "" {
		if cwd, err := os.Readlink(fmt.Sprintf("/proc/%d/cwd", pid)); err == nil {
			path = cwd
		}
	}
	if path == "" {
		return errors.New("this process location is not readable")
	}
	cmd := exec.Command("xdg-open", path)
	cmd.Stdin = nil
	cmd.Stdout = nil
	cmd.Stderr = nil
	return cmd.Start()
}
