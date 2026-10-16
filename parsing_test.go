package main

import (
	"bufio"
	"bytes"
	"fmt"
	"os"
	"reflect"
	"strconv"
	"strings"
	"testing"
)

func referenceStatus(raw []byte) procStatus {
	var s procStatus
	values := map[string]uint64{}
	sc := bufio.NewScanner(bytes.NewReader(raw))
	for sc.Scan() {
		fields := strings.Fields(sc.Text())
		if len(fields) < 2 {
			continue
		}
		key := strings.TrimSuffix(fields[0], ":")
		if key == "Uid" {
			u, _ := strconv.ParseUint(fields[1], 10, 32)
			s.uid = uint32(u)
			continue
		}
		if key == "Threads" {
			s.threads, _ = strconv.ParseUint(fields[1], 10, 64)
			continue
		}
		if strings.HasPrefix(key, "Vm") || strings.HasPrefix(key, "Rss") {
			v, _ := strconv.ParseUint(fields[1], 10, 64)
			values[key] = v * 1024
		}
	}
	s.rss = values["VmRSS"]
	priv := values["RssAnon"] + values["RssShmem"]
	if priv == 0 {
		priv = s.rss
	}
	s.private = priv
	if v, ok := values["RssAnon"]; ok {
		s.anon = u64p(v)
	}
	if v, ok := values["RssFile"]; ok {
		s.file = u64p(v)
	}
	if v, ok := values["RssShmem"]; ok {
		s.shmem = u64p(v)
	}
	if v, ok := values["VmSwap"]; ok {
		s.swap = u64p(v)
	}
	return s
}

func same(a, b *uint64) bool {
	if a == nil || b == nil {
		return a == nil && b == nil
	}
	return *a == *b
}

func TestTheStatusReaderAgreesWithTheOneItReplaced(t *testing.T) {
	entries, err := os.ReadDir("/proc")
	if err != nil {
		t.Skip("no /proc on this machine")
	}
	checked := 0
	for _, e := range entries {
		pid, err := strconv.Atoi(e.Name())
		if err != nil {
			continue
		}
		// Both parsers read the same bytes. Reading the file twice would
		// compare two moments of a running process, not two parsers.
		raw, err := os.ReadFile(fmt.Sprintf("/proc/%d/status", pid))
		if err != nil {
			continue
		}
		got := parseStatus(raw)
		want := referenceStatus(raw)
		if want.threads == 0 {
			continue
		}
		checked++
		if got.uid != want.uid || got.threads != want.threads || got.rss != want.rss || got.private != want.private {
			t.Errorf("pid %d: got uid=%d threads=%d rss=%d private=%d, wanted uid=%d threads=%d rss=%d private=%d",
				pid, got.uid, got.threads, got.rss, got.private, want.uid, want.threads, want.rss, want.private)
		}
		if !same(got.anon, want.anon) || !same(got.file, want.file) || !same(got.shmem, want.shmem) || !same(got.swap, want.swap) {
			t.Errorf("pid %d: the memory composition differs", pid)
		}
	}
	if checked < 10 {
		t.Fatalf("only %d processes were compared, which proves little", checked)
	}
	t.Logf("%d Prozesse verglichen", checked)
}

func TestTheStatLineIsSplitTheSameWay(t *testing.T) {
	entries, err := os.ReadDir("/proc")
	if err != nil {
		t.Skip("no /proc on this machine")
	}
	checked := 0
	for _, e := range entries {
		pid, err := strconv.Atoi(e.Name())
		if err != nil {
			continue
		}
		raw, err := os.ReadFile(fmt.Sprintf("/proc/%d/stat", pid))
		if err != nil {
			continue
		}
		line := string(raw)
		close := strings.LastIndex(line, ") ")
		if close < 0 {
			continue
		}
		want := strings.Fields(line[close+2:])
		if len(want) < 37 {
			continue
		}
		var got [37]string
		if !statFields(line[close+2:], &got) {
			t.Errorf("pid %d: the stat line was refused although it has %d fields", pid, len(want))
			continue
		}
		checked++
		if !reflect.DeepEqual(got[:], want[:37]) {
			t.Errorf("pid %d: fields differ\n got %v\nwant %v", pid, got[:], want[:37])
		}
	}
	if checked < 10 {
		t.Fatalf("only %d stat lines were compared, which proves little", checked)
	}
	t.Logf("%d stat-Zeilen verglichen", checked)
}

func TestAShortStatLineIsRefused(t *testing.T) {
	var f [37]string
	if statFields("1 2 3", &f) {
		t.Error("a line with three fields was accepted as a full stat line")
	}
}
