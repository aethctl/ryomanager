package main

import (
	"fmt"
	"os"
	"strconv"
	"testing"
	"time"
)

func fakeMonitor(i int) string {
	if i == 1 {
		return "HDMI-A-1"
	}
	return strconv.Itoa(i)
}

func TestHyprlandWindowsAreGroupedByOwningProcess(t *testing.T) {
	out := []byte(`[
	  {"address":"0x55a","title":"ryomanager","class":"ryomanager","pid":4242,"monitor":1,"mapped":true,"focusHistoryID":0,"workspace":{"id":2,"name":"2"}},
	  {"address":"0x55b","title":"second window","class":"ryomanager","pid":4242,"monitor":0,"mapped":true,"focusHistoryID":3,"workspace":{"id":1,"name":"1"}},
	  {"address":"0x55c","title":"zen","class":"zen","pid":99,"monitor":0,"mapped":true,"focusHistoryID":1,"workspace":{"id":1,"name":"1"}}
	]`)
	windows, ok := parseHyprlandWindows(out, fakeMonitor)
	if !ok {
		t.Fatal("hyprctl output was not read")
	}
	if len(windows) != 2 {
		t.Fatalf("two processes own windows here, got %d", len(windows))
	}
	mine := windows[4242]
	if len(mine) != 2 {
		t.Fatalf("both windows of 4242 belong to it, got %d", len(mine))
	}
	if mine[0].ID != "0x55a" || mine[0].Title != "ryomanager" || mine[0].AppID != "ryomanager" {
		t.Errorf("window read wrong: %+v", mine[0])
	}
	if mine[0].Workspace != "2" {
		t.Errorf("workspace is the name a person reads, got %q", mine[0].Workspace)
	}
	if mine[0].Output != "HDMI-A-1" {
		t.Errorf("monitor index was not resolved to its name, got %q", mine[0].Output)
	}
	if !mine[0].Focused {
		t.Error("focusHistoryID 0 is the window in front")
	}
	if mine[1].Focused {
		t.Error("only one window is in front")
	}
}

func TestAnUnmappedWindowIsNotAWindow(t *testing.T) {
	out := []byte(`[{"address":"0x1","title":"","class":"x","pid":7,"monitor":0,"mapped":false,"focusHistoryID":0,"workspace":{"name":"1"}}]`)
	windows, ok := parseHyprlandWindows(out, fakeMonitor)
	if !ok {
		t.Fatal("hyprctl output was not read")
	}
	if len(windows) != 0 {
		t.Fatalf("a window that is not mapped is not on screen, got %v", windows)
	}
}

func TestGarbageFromTheCompositorIsNotAnEmptyDesktop(t *testing.T) {
	if _, ok := parseHyprlandWindows([]byte("hyprctl: command not found"), fakeMonitor); ok {
		t.Error("unparseable output must not read as a session with no windows")
	}
	if _, ok := parseNiriWindows([]byte(""), nil); ok {
		t.Error("unparseable output must not read as a session with no windows")
	}
}

func TestNiriWindowsTakeTheWorkspaceAPersonSees(t *testing.T) {
	spaces := parseNiriWorkspaces([]byte(`[
	  {"id":9,"idx":3,"name":null,"output":"eDP-1","is_active":true,"is_focused":true},
	  {"id":11,"idx":1,"name":"mail","output":"HDMI-A-1","is_active":false,"is_focused":false}
	]`))
	out := []byte(`[
	  {"id":12,"title":"ryomanager","app_id":"ryomanager","pid":4242,"workspace_id":9,"is_focused":true},
	  {"id":14,"title":"thunderbird","app_id":"mail","pid":51,"workspace_id":11,"is_focused":false},
	  {"id":15,"title":"stale","app_id":"x","pid":52,"workspace_id":999,"is_focused":false},
	  {"id":13,"title":"no pid","app_id":"ghost","pid":null,"workspace_id":9,"is_focused":false}
	]`)
	windows, ok := parseNiriWindows(out, spaces)
	if !ok {
		t.Fatal("niri output was not read")
	}
	if len(windows) != 3 {
		t.Fatalf("only the windows with a process behind them can be linked, got %d", len(windows))
	}
	w := windows[4242][0]
	if w.ID != "12" || !w.Focused {
		t.Errorf("window read wrong: %+v", w)
	}
	if w.Workspace != "3" {
		t.Errorf("an unnamed workspace is the number in the bar, not niri's internal id: %q", w.Workspace)
	}
	if w.Output != "eDP-1" {
		t.Errorf("the window sits on the workspace's screen, got %q", w.Output)
	}
	if got := windows[51][0].Workspace; got != "mail" {
		t.Errorf("a named workspace keeps its name, got %q", got)
	}
	if got := windows[52][0].Workspace; got != "" {
		t.Errorf("a workspace nobody knows is left blank, not printed as an id: %q", got)
	}
}

func TestAMonitorNobodyKnowsIsLeftBlank(t *testing.T) {
	if os.Getenv("HYPRLAND_INSTANCE_SIGNATURE") == "" {
		t.Skip("no Hyprland session to ask for monitors")
	}
	monitorsMu.Lock()
	monitorNames, monitorsAt = nil, time.Time{}
	monitorsMu.Unlock()

	if name := monitorName(0); name == "" {
		t.Fatal("the first monitor of a running session has a name")
	}
	if got := monitorName(4242); got != "" {
		t.Errorf("an index no monitor holds must not be shown as a screen name, got %q", got)
	}
}

func TestMonitorNamesAreReadFromHyprctl(t *testing.T) {
	names := parseHyprlandMonitors([]byte(`[{"id":0,"name":"DP-1"},{"id":1,"name":"HDMI-A-1"}]`))
	if names[0] != "DP-1" || names[1] != "HDMI-A-1" {
		t.Errorf("monitors read wrong: %v", names)
	}
	if parseHyprlandMonitors([]byte("not json")) != nil {
		t.Error("unparseable output must not be stored as the list of monitors")
	}
}

func TestWindowsReachTheMemberAndTheGroup(t *testing.T) {
	groups := []ProcessGroup{{
		Key:     "app:zen",
		Windows: []WindowRef{},
		Members: []ProcessInfo{
			{PID: 10, Windows: []WindowRef{}},
			{PID: 11, Windows: []WindowRef{}},
		},
	}}
	attachWindows(groups, map[int][]WindowRef{
		10: {{ID: "a"}},
		11: {{ID: "b"}, {ID: "c"}},
		12: {{ID: "elsewhere"}},
	})
	if len(groups[0].Windows) != 3 {
		t.Fatalf("the group carries every window of its members, got %d", len(groups[0].Windows))
	}
	if len(groups[0].Members[0].Windows) != 1 || groups[0].Members[0].Windows[0].ID != "a" {
		t.Errorf("a member carries only its own windows: %+v", groups[0].Members[0].Windows)
	}
	if len(groups[0].Members[1].Windows) != 2 {
		t.Errorf("a member with two windows keeps both, got %d", len(groups[0].Members[1].Windows))
	}
}

func TestNothingToLinkTouchesNoGroup(t *testing.T) {
	groups := []ProcessGroup{{Key: "k", Windows: []WindowRef{}, Members: []ProcessInfo{{PID: 10, Windows: []WindowRef{}}}}}
	before := fmt.Sprintf("%+v", groups)
	attachWindows(groups, nil)
	attachWindows(groups, map[int][]WindowRef{99: {{ID: "elsewhere"}}})
	if after := fmt.Sprintf("%+v", groups); after != before {
		t.Errorf("a group with no windows of its own was changed:\n  was %s\n  now %s", before, after)
	}
}

func TestTheRunningCompositorAnswers(t *testing.T) {
	if os.Getenv("HYPRLAND_INSTANCE_SIGNATURE") == "" && os.Getenv("NIRI_SOCKET") == "" {
		t.Skip("no Hyprland or niri session to ask")
	}
	windows, source := readWindows()
	if source != "shell" {
		t.Fatalf("a running compositor must answer, got %q", source)
	}
	if len(windows) == 0 {
		t.Fatal("a session with a window open reported none")
	}
	for pid, refs := range windows {
		for _, w := range refs {
			t.Logf("pid %d: %q class=%q ws=%q out=%q focused=%v", pid, w.Title, w.AppID, w.Workspace, w.Output, w.Focused)
		}
	}
}

func TestASnapshotCarriesTheWindowsItFound(t *testing.T) {
	if os.Getenv("HYPRLAND_INSTANCE_SIGNATURE") == "" && os.Getenv("NIRI_SOCKET") == "" {
		t.Skip("no Hyprland or niri session to ask")
	}
	snap := NewCollector().Snapshot()
	if snap.WindowsSource != "shell" {
		t.Fatalf("the snapshot must say where the windows came from, got %q", snap.WindowsSource)
	}
	for _, g := range snap.Groups {
		if g.Windows == nil {
			t.Fatalf("%s carries no window list at all; the page reads its length", g.Name)
		}
		for _, m := range g.Members {
			if m.Windows == nil {
				t.Fatalf("pid %d carries no window list at all; the page reads its length", m.PID)
			}
		}
	}
	linked := 0
	for _, g := range snap.Groups {
		if len(g.Windows) == 0 {
			continue
		}
		linked++
		members := 0
		for _, m := range g.Members {
			members += len(m.Windows)
		}
		if members != len(g.Windows) {
			t.Errorf("%s holds %d windows but its members hold %d", g.Name, len(g.Windows), members)
		}
		t.Logf("%s: %d window(s), first %q", g.Name, len(g.Windows), g.Windows[0].Title)
	}
	if linked == 0 {
		t.Fatal("no group was given a window")
	}
}

func TestAWedgedCompositorDoesNotHoldUpTheSnapshot(t *testing.T) {
	was := compositorTimeout
	compositorTimeout = 100 * time.Millisecond
	t.Cleanup(func() { compositorTimeout = was })

	began := time.Now()
	if _, ok := askCompositor("sleep", "10"); ok {
		t.Fatal("a command that never answered was reported as an answer")
	}
	if waited := time.Since(began); waited > time.Second {
		t.Errorf("waited %v on a compositor that never answered, with the collector lock held", waited)
	}
}
