package main

import (
	"context"
	"errors"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
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

func validateIdentity(pid int, start uint64) error {
	if pid <= 1 || pid == os.Getpid() {
		return errors.New("RyoManager will not terminate this protected process")
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
	for _, ref := range members {
		if err := validateIdentity(int(ref[0]), ref[1]); err != nil {
			return err
		}
	}
	for _, ref := range members {
		sig := syscall.SIGTERM
		if force {
			sig = syscall.SIGKILL
		}
		if err := syscall.Kill(int(ref[0]), sig); err != nil {
			return err
		}
	}
	return nil
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
