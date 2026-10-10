package main

import (
	"context"
	"errors"
	"os"
	"syscall"
)

type App struct {
	ctx       context.Context
	collector *Collector
}

func NewApp() *App {
	return &App{collector: NewCollector()}
}

func (a *App) startup(ctx context.Context) {
	a.ctx = ctx
}

func (a *App) Snapshot() Snapshot {
	return a.collector.Snapshot()
}

func (a *App) EndProcess(pid int, force bool) error {
	if pid <= 1 || pid == os.Getpid() {
		return errors.New("RyoManager will not terminate this protected process")
	}

	signal := syscall.SIGTERM
	if force {
		signal = syscall.SIGKILL
	}

	if err := syscall.Kill(pid, signal); err != nil {
		return err
	}
	return nil
}
