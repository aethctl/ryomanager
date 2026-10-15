package main

import (
	"context"
	"encoding/json"
	"os"
	"os/exec"
	"strconv"
	"sync"
	"time"
)

var compositorTimeout = 2 * time.Second

func askCompositor(name string, args ...string) ([]byte, bool) {
	ctx, cancel := context.WithTimeout(context.Background(), compositorTimeout)
	defer cancel()
	out, err := exec.CommandContext(ctx, name, args...).Output()
	if err != nil {
		return nil, false
	}
	return out, true
}

func readWindows() (map[int][]WindowRef, string) {
	if os.Getenv("HYPRLAND_INSTANCE_SIGNATURE") != "" {
		if windows, ok := readHyprlandWindows(); ok {
			return windows, "shell"
		}
	}
	if os.Getenv("NIRI_SOCKET") != "" {
		if windows, ok := readNiriWindows(); ok {
			return windows, "shell"
		}
	}
	return nil, "none"
}

type hyprClient struct {
	Address   string `json:"address"`
	Title     string `json:"title"`
	Class     string `json:"class"`
	PID       int    `json:"pid"`
	Monitor   int    `json:"monitor"`
	Mapped    bool   `json:"mapped"`
	FocusRank int    `json:"focusHistoryID"`
	Workspace struct {
		Name string `json:"name"`
	} `json:"workspace"`
}

func readHyprlandWindows() (map[int][]WindowRef, bool) {
	out, ok := askCompositor("hyprctl", "clients", "-j")
	if !ok {
		return nil, false
	}
	return parseHyprlandWindows(out, monitorName)
}

func parseHyprlandWindows(out []byte, name func(int) string) (map[int][]WindowRef, bool) {
	var clients []hyprClient
	if err := json.Unmarshal(out, &clients); err != nil {
		return nil, false
	}
	windows := map[int][]WindowRef{}
	for _, c := range clients {
		if c.PID <= 0 || !c.Mapped {
			continue
		}
		windows[c.PID] = append(windows[c.PID], WindowRef{
			ID:        c.Address,
			Title:     c.Title,
			AppID:     c.Class,
			Workspace: c.Workspace.Name,
			Output:    name(c.Monitor),
			Focused:   c.FocusRank == 0,
		})
	}
	return windows, true
}

var (
	monitorsMu   sync.Mutex
	monitorNames map[int]string
	monitorsAt   time.Time
)

const monitorsTTL = 30 * time.Second

func monitorName(index int) string {
	monitorsMu.Lock()
	defer monitorsMu.Unlock()
	if monitorNames == nil || time.Since(monitorsAt) > monitorsTTL {
		monitorsAt = time.Now()
		names := map[int]string{}
		if out, ok := askCompositor("hyprctl", "monitors", "-j"); ok {
			if parsed := parseHyprlandMonitors(out); parsed != nil {
				names = parsed
			}
		}
		monitorNames = names
	}
	return monitorNames[index]
}

func parseHyprlandMonitors(out []byte) map[int]string {
	var monitors []struct {
		ID   int    `json:"id"`
		Name string `json:"name"`
	}
	if json.Unmarshal(out, &monitors) != nil {
		return nil
	}
	names := map[int]string{}
	for _, m := range monitors {
		names[m.ID] = m.Name
	}
	return names
}

type niriWindow struct {
	ID          uint64  `json:"id"`
	Title       string  `json:"title"`
	AppID       string  `json:"app_id"`
	PID         int     `json:"pid"`
	WorkspaceID *uint64 `json:"workspace_id"`
	IsFocused   bool    `json:"is_focused"`
}

func readNiriWindows() (map[int][]WindowRef, bool) {
	out, ok := askCompositor("niri", "msg", "--json", "windows")
	if !ok {
		return nil, false
	}
	return parseNiriWindows(out, niriWorkspaceNames())
}

type niriWorkspace struct {
	ID     uint64  `json:"id"`
	Idx    int     `json:"idx"`
	Name   *string `json:"name"`
	Output *string `json:"output"`
}

type workspacePlace struct{ label, output string }

func niriWorkspaceNames() map[uint64]workspacePlace {
	out, ok := askCompositor("niri", "msg", "--json", "workspaces")
	if !ok {
		return nil
	}
	return parseNiriWorkspaces(out)
}

func parseNiriWorkspaces(out []byte) map[uint64]workspacePlace {
	var list []niriWorkspace
	if json.Unmarshal(out, &list) != nil {
		return nil
	}
	places := map[uint64]workspacePlace{}
	for _, w := range list {
		place := workspacePlace{label: strconv.Itoa(w.Idx)}
		if w.Name != nil && *w.Name != "" {
			place.label = *w.Name
		}
		if w.Output != nil {
			place.output = *w.Output
		}
		places[w.ID] = place
	}
	return places
}

func parseNiriWindows(out []byte, places map[uint64]workspacePlace) (map[int][]WindowRef, bool) {
	var list []niriWindow
	if err := json.Unmarshal(out, &list); err != nil {
		return nil, false
	}
	windows := map[int][]WindowRef{}
	for _, w := range list {
		if w.PID <= 0 {
			continue
		}
		ref := WindowRef{
			ID:      strconv.FormatUint(w.ID, 10),
			Title:   w.Title,
			AppID:   w.AppID,
			Focused: w.IsFocused,
		}
		if w.WorkspaceID != nil {
			if place, ok := places[*w.WorkspaceID]; ok {
				ref.Workspace = place.label
				ref.Output = place.output
			}
		}
		windows[w.PID] = append(windows[w.PID], ref)
	}
	return windows, true
}

func attachWindows(groups []ProcessGroup, windows map[int][]WindowRef) {
	if len(windows) == 0 {
		return
	}
	for gi := range groups {
		g := &groups[gi]
		for mi := range g.Members {
			refs := windows[g.Members[mi].PID]
			if len(refs) == 0 {
				continue
			}
			g.Members[mi].Windows = refs
			g.Windows = append(g.Windows, refs...)
		}
	}
}
