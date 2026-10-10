package main

import (
	"bufio"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"os/user"
	"path/filepath"
	"runtime"
	"sort"
	"strconv"
	"strings"
	"sync"
	"syscall"
	"time"
)

const fallbackAccent = "#7898a5"

type cpuRaw struct{ total, idle, user, system, iowait, irq uint64 }
type procRaw struct{ ticks, utime, stime, read, write uint64 }
type diskRaw struct{ readSectors, writeSectors, readOps, writeOps, busyMS uint64 }
type netRaw struct{ rx, tx uint64 }

type Collector struct {
	mu        sync.Mutex
	lastAt    time.Time
	lastCPU   cpuRaw
	lastCores []cpuRaw
	lastProc  map[int]procRaw
	lastDisk  map[string]diskRaw
	lastNet   map[string]netRaw
	hasSample bool
	groups    map[string][][2]uint64
	processes map[[2]uint64]ProcessInfo
	icons     map[string]string
}

func NewCollector() *Collector {
	return &Collector{lastProc: map[int]procRaw{}, lastDisk: map[string]diskRaw{}, lastNet: map[string]netRaw{}, groups: map[string][][2]uint64{}, processes: map[[2]uint64]ProcessInfo{}, icons: map[string]string{}}
}

func (c *Collector) Snapshot() Snapshot {
	c.mu.Lock()
	defer c.mu.Unlock()
	now := time.Now()
	sampleMS := 1000.0
	if !c.lastAt.IsZero() {
		sampleMS = float64(now.Sub(c.lastAt).Microseconds()) / 1000
	}
	if sampleMS <= 0 {
		sampleMS = 1000
	}
	c.lastAt = now
	global, cores := readCPUStats()
	processes, procNow := readProcesses(c.lastProc, global.total-c.lastCPU.total, sampleMS, c.hasSample)
	cpu := buildCPUInfo(global, cores, c.lastCPU, c.lastCores, processes, c.hasSample)
	memory := readMemoryInfo()
	disks, diskNow := readDisks(c.lastDisk, sampleMS, c.hasSample)
	networks, netNow := readNetworks(c.lastNet, sampleMS, c.hasSample)
	gpus := readGPUs()
	energy := readEnergy()
	thermal := readThermals()
	groups, iconMap := groupProcesses(processes)
	c.groups = map[string][][2]uint64{}
	c.processes = map[[2]uint64]ProcessInfo{}
	for _, g := range groups {
		refs := make([][2]uint64, 0, len(g.Members))
		for _, p := range g.Members {
			refs = append(refs, [2]uint64{uint64(p.PID), p.StartTime})
			c.processes[[2]uint64{uint64(p.PID), p.StartTime}] = p
		}
		c.groups[g.Key] = refs
	}
	c.icons = iconMap
	c.lastCPU, c.lastCores, c.lastProc, c.lastDisk, c.lastNet = global, cores, procNow, diskNow, netNow
	c.hasSample = true
	limits := Availability{
		"io":                   "I/O counters are readable only for your own processes",
		"pss":                  "exact memory and counters are read for the inspected process",
		"gpu.process":          "per-process GPU usage is unavailable when the driver does not expose engine time",
		"network.processBytes": "Linux keeps no per-process network byte counters",
		"connections":          "exact memory and counters are read for the inspected process",
		"fds":                  "exact memory and counters are read for the inspected process",
		"ctxSwitches":          "exact memory and counters are read for the inspected process",
		"oomScore":             "exact memory and counters are read for the inspected process",
	}
	return Snapshot{TimestampMS: now.UnixMilli(), SampleMS: sampleMS, Accent: ryokuAccent(), WindowsSource: "none", SelfPID: os.Getpid(), CPU: cpu, Memory: memory, Disks: disks, Networks: networks, GPUs: gpus, Energy: energy, Thermal: thermal, ProcessCount: len(processes), Groups: groups, System: readSystemInfo(cpu), Limits: limits}
}

func (c *Collector) Process(pid int, start uint64) (ProcessInfo, bool) {
	c.mu.Lock()
	defer c.mu.Unlock()
	p, ok := c.processes[[2]uint64{uint64(pid), start}]
	return p, ok
}

func (c *Collector) GroupMembers(key string) ([][2]uint64, bool) {
	c.mu.Lock()
	defer c.mu.Unlock()
	v, ok := c.groups[key]
	return append([][2]uint64(nil), v...), ok
}

func (c *Collector) AppIcon(key string) *string {
	c.mu.Lock()
	defer c.mu.Unlock()
	path := c.icons[key]
	if path == "" {
		return nil
	}
	raw, err := os.ReadFile(path)
	if err != nil {
		return nil
	}
	ext := strings.ToLower(filepath.Ext(path))
	mime := "image/png"
	if ext == ".svg" {
		mime = "image/svg+xml"
	}
	if ext == ".jpg" || ext == ".jpeg" {
		mime = "image/jpeg"
	}
	value := "data:" + mime + ";base64," + base64.StdEncoding.EncodeToString(raw)
	return &value
}

func readCPUStats() (cpuRaw, []cpuRaw) {
	f, err := os.Open("/proc/stat")
	if err != nil {
		return cpuRaw{}, nil
	}
	defer f.Close()
	var global cpuRaw
	var cores []cpuRaw
	s := bufio.NewScanner(f)
	for s.Scan() {
		fields := strings.Fields(s.Text())
		if len(fields) < 5 || !strings.HasPrefix(fields[0], "cpu") {
			continue
		}
		r := parseCPULine(fields)
		if fields[0] == "cpu" {
			global = r
		} else {
			cores = append(cores, r)
		}
	}
	return global, cores
}

func parseCPULine(fields []string) cpuRaw {
	vals := make([]uint64, 10)
	for i := 1; i < len(fields) && i <= 10; i++ {
		vals[i-1], _ = strconv.ParseUint(fields[i], 10, 64)
	}
	var total uint64
	for _, v := range vals {
		total += v
	}
	return cpuRaw{total: total, idle: vals[3] + vals[4], user: vals[0] + vals[1], system: vals[2], iowait: vals[4], irq: vals[5] + vals[6]}
}

func pct(now, before, totalNow, totalBefore uint64) float64 {
	if totalNow <= totalBefore || now < before {
		return 0
	}
	return float64(now-before) / float64(totalNow-totalBefore) * 100
}

func buildCPUInfo(now cpuRaw, cores []cpuRaw, prev cpuRaw, prevCores []cpuRaw, processes []ProcessInfo, sampled bool) CpuInfo {
	usage, userP, systemP, iowaitP, irqP := 0.0, 0.0, 0.0, 0.0, 0.0
	if sampled {
		usage = pct(now.total-now.idle, prev.total-prev.idle, now.total, prev.total)
		userP = pct(now.user, prev.user, now.total, prev.total)
		systemP = pct(now.system, prev.system, now.total, prev.total)
		iowaitP = pct(now.iowait, prev.iowait, now.total, prev.total)
		irqP = pct(now.irq, prev.irq, now.total, prev.total)
	}
	coreInfo := make([]CpuCore, 0, len(cores))
	var avgFreq float64
	var freqCount int
	for i, r := range cores {
		u := 0.0
		if sampled && i < len(prevCores) {
			u = pct(r.total-r.idle, prevCores[i].total-prevCores[i].idle, r.total, prevCores[i].total)
		}
		freq := readFloat(fmt.Sprintf("/sys/devices/system/cpu/cpu%d/cpufreq/scaling_cur_freq", i))
		var fp *float64
		if freq != nil {
			v := *freq / 1000
			fp = &v
			avgFreq += v
			freqCount++
		}
		coreInfo = append(coreInfo, CpuCore{Index: uint32(i), Usage: u, FreqMhz: fp})
	}
	var avgFreqP *float64
	if freqCount > 0 {
		v := avgFreq / float64(freqCount)
		avgFreqP = &v
	}
	maxFreq := readFloat("/sys/devices/system/cpu/cpu0/cpufreq/cpuinfo_max_freq")
	if maxFreq != nil {
		v := *maxFreq / 1000
		maxFreq = &v
	}
	gov := readStringPtr("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor")
	load := [3]float64{}
	if b, e := os.ReadFile("/proc/loadavg"); e == nil {
		f := strings.Fields(string(b))
		for i := 0; i < 3 && i < len(f); i++ {
			load[i], _ = strconv.ParseFloat(f[i], 64)
		}
	}
	uptime := 0.0
	if b, e := os.ReadFile("/proc/uptime"); e == nil {
		f := strings.Fields(string(b))
		if len(f) > 0 {
			uptime, _ = strconv.ParseFloat(f[0], 64)
		}
	}
	var threads uint64
	for _, p := range processes {
		threads += p.Threads
	}
	open, max := readOpenFiles()
	return CpuInfo{Usage: usage, User: userP, System: systemP, IOWait: iowaitP, IRQ: irqP, Cores: coreInfo, FreqMhz: avgFreqP, MaxFreqMhz: maxFreq, Governor: gov, LoadAvg: load, UptimeSeconds: uptime, Processes: uint64(len(processes)), Threads: threads, OpenFiles: open, OpenFilesMax: max, Model: readCPUModel(), Sockets: uint64(socketCount()), PhysicalCores: uint64(physicalCoreCount()), LogicalCPUs: uint64(runtime.NumCPU()), Virtualization: readVirtualization(), Caches: readCaches(), Pressure: readPressure("/proc/pressure/cpu")}
}
func readProcesses(prev map[int]procRaw, deltaTotal uint64, sampleMS float64, sampled bool) ([]ProcessInfo, map[int]procRaw) {
	entries, err := os.ReadDir("/proc")
	if err != nil {
		return nil, map[int]procRaw{}
	}
	out := make([]ProcessInfo, 0, len(entries))
	current := map[int]procRaw{}
	for _, e := range entries {
		if !e.IsDir() {
			continue
		}
		pid, err := strconv.Atoi(e.Name())
		if err != nil {
			continue
		}
		raw, err := os.ReadFile(filepath.Join("/proc", e.Name(), "stat"))
		if err != nil {
			continue
		}
		p, pr, ok := parseProcess(pid, string(raw))
		if !ok {
			continue
		}
		status := readProcStatus(pid)
		p.UID = status.uid
		p.User = userName(status.uid)
		p.RSS = status.rss
		p.Memory = status.private
		p.MemoryKind = "private"
		p.RSSAnon = status.anon
		p.RSSFile = status.file
		p.RSSShmem = status.shmem
		p.Swap = status.swap
		p.Threads = status.threads
		p.CtxSwitches = nil
		p.OOMScore = nil
		p.Exe = readLinkPtr(fmt.Sprintf("/proc/%d/exe", pid))
		p.Cwd = readLinkPtr(fmt.Sprintf("/proc/%d/cwd", pid))
		p.Command = readCmdline(pid)
		p.KernelThread = p.Command == "" && p.Exe == nil
		p.Unit = readUnit(pid)
		p.Windows = []WindowRef{}
		io := readProcIO(pid)
		pr.read = io.read
		pr.write = io.write
		if io.ok {
			r, w := float64(0), float64(0)
			if sampled {
				if old, ok := prev[pid]; ok && sampleMS > 0 {
					r = float64(safeDelta(pr.read, old.read)) / (sampleMS / 1000)
					w = float64(safeDelta(pr.write, old.write)) / (sampleMS / 1000)
				}
			}
			p.DiskRead = &r
			p.DiskWrite = &w
			rr, ww := pr.read, pr.write
			p.DiskReadTotal = &rr
			p.DiskWriteTotal = &ww
		}
		if sampled && deltaTotal > 0 {
			if old, ok := prev[pid]; ok {
				p.CPU = float64(safeDelta(pr.ticks, old.ticks)) / float64(deltaTotal) * float64(runtime.NumCPU()) * 100
				p.CPUUser = float64(safeDelta(pr.utime, old.utime)) / float64(deltaTotal) * float64(runtime.NumCPU()) * 100
				p.CPUSystem = float64(safeDelta(pr.stime, old.stime)) / float64(deltaTotal) * float64(runtime.NumCPU()) * 100
			}
		}
		p.EnergyScore = p.CPU
		if p.DiskRead != nil {
			p.EnergyScore += (*p.DiskRead + *p.DiskWrite) / 10_000_000
		}
		p.Energy = energyBand(p.EnergyScore)
		current[pid] = pr
		out = append(out, p)
	}
	sort.Slice(out, func(i, j int) bool {
		if out[i].CPU == out[j].CPU {
			return out[i].Memory > out[j].Memory
		}
		return out[i].CPU > out[j].CPU
	})
	return out, current
}

type procStatus struct {
	uid                     uint32
	rss, private            uint64
	anon, file, shmem, swap *uint64
	threads                 uint64
}

func readProcStatus(pid int) procStatus {
	var s procStatus
	values := map[string]uint64{}
	f, e := os.Open(fmt.Sprintf("/proc/%d/status", pid))
	if e != nil {
		return s
	}
	defer f.Close()
	sc := bufio.NewScanner(f)
	for sc.Scan() {
		line := sc.Text()
		fields := strings.Fields(line)
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

func parseProcess(pid int, raw string) (ProcessInfo, procRaw, bool) {
	open := strings.IndexByte(raw, '(')
	close := strings.LastIndex(raw, ") ")
	if open < 0 || close < 0 {
		return ProcessInfo{}, procRaw{}, false
	}
	name := raw[open+1 : close]
	f := strings.Fields(raw[close+2:])
	if len(f) < 37 {
		return ProcessInfo{}, procRaw{}, false
	}
	ppid, _ := strconv.Atoi(f[1])
	ut, _ := strconv.ParseUint(f[11], 10, 64)
	st, _ := strconv.ParseUint(f[12], 10, 64)
	pri, _ := strconv.ParseInt(f[15], 10, 64)
	nice, _ := strconv.ParseInt(f[16], 10, 64)
	threads, _ := strconv.ParseUint(f[17], 10, 64)
	start, _ := strconv.ParseUint(f[19], 10, 64)
	virt, _ := strconv.ParseUint(f[20], 10, 64)
	procCPU, _ := strconv.ParseUint(f[36], 10, 32)
	p := ProcessInfo{PID: pid, ParentPID: &ppid, StartTime: start, Name: name, State: stateName(f[0]), Virtual: virt, Priority: pri, Nice: nice, Threads: threads, LastCPU: u32p(uint32(procCPU)), MemoryKind: "private", Energy: "none", Windows: []WindowRef{}}
	return p, procRaw{ticks: ut + st, utime: ut, stime: st}, true
}

func readProcIO(pid int) (r struct {
	read, write uint64
	ok          bool
}) {
	b, e := os.ReadFile(fmt.Sprintf("/proc/%d/io", pid))
	if e != nil {
		return
	}
	for _, line := range strings.Split(string(b), "\n") {
		k, v, ok := strings.Cut(line, ":")
		if !ok {
			continue
		}
		n, _ := strconv.ParseUint(strings.TrimSpace(v), 10, 64)
		if k == "read_bytes" {
			r.read = n
			r.ok = true
		}
		if k == "write_bytes" {
			r.write = n
			r.ok = true
		}
	}
	return
}
func readCmdline(pid int) string {
	b, e := os.ReadFile(fmt.Sprintf("/proc/%d/cmdline", pid))
	if e != nil || len(b) == 0 {
		return ""
	}
	return strings.Join(strings.Split(strings.TrimRight(string(b), "\x00"), "\x00"), " ")
}
func readUnit(pid int) *string {
	b, e := os.ReadFile(fmt.Sprintf("/proc/%d/cgroup", pid))
	if e != nil {
		return nil
	}
	for _, l := range strings.Split(string(b), "\n") {
		if _, path, ok := strings.Cut(l, "::"); ok {
			for _, part := range strings.Split(path, "/") {
				if strings.HasSuffix(part, ".service") || strings.HasSuffix(part, ".scope") {
					v := part
					return &v
				}
			}
		}
	}
	return nil
}
func processStartTime(pid int) (uint64, bool) {
	b, e := os.ReadFile(fmt.Sprintf("/proc/%d/stat", pid))
	if e != nil {
		return 0, false
	}
	_, _, ok := parseProcess(pid, string(b))
	if !ok {
		return 0, false
	}
	return prStart(pid, string(b))
}
func prStart(pid int, raw string) (uint64, bool) {
	close := strings.LastIndex(raw, ") ")
	if close < 0 {
		return 0, false
	}
	f := strings.Fields(raw[close+2:])
	if len(f) < 20 {
		return 0, false
	}
	v, e := strconv.ParseUint(f[19], 10, 64)
	return v, e == nil
}
func stateName(v string) string {
	if v == "" {
		return "unknown"
	}
	switch v[0] {
	case 'R':
		return "running"
	case 'S':
		return "sleeping"
	case 'D':
		return "waiting"
	case 'T', 't':
		return "stopped"
	case 'Z':
		return "zombie"
	case 'I':
		return "idle"
	case 'X', 'x':
		return "dead"
	}
	return "unknown"
}
func energyBand(v float64) string {
	switch {
	case v < 0.5:
		return "none"
	case v < 2:
		return "very-low"
	case v < 5:
		return "low"
	case v < 15:
		return "moderate"
	case v < 40:
		return "high"
	default:
		return "very-high"
	}
}
func safeDelta(a, b uint64) uint64 {
	if a >= b {
		return a - b
	}
	return 0
}
func groupProcesses(processes []ProcessInfo) ([]ProcessGroup, map[string]string) {
	desktops := scanDesktopEntries()
	groups := map[string]*ProcessGroup{}
	icons := map[string]string{}
	for _, p := range processes {
		key, name, subtitle, category := "", "", "", "background"
		var iconKey *string
		exeBase := ""
		if p.Exe != nil {
			exeBase = filepath.Base(*p.Exe)
		}
		if d, ok := desktops[exeBase]; ok {
			key = "app:" + d.id
			name = d.name
			subtitle = exeBase
			category = "apps"
			k := key
			iconKey = &k
			if d.icon != "" {
				icons[key] = d.icon
			}
		} else if p.Unit != nil {
			key = "unit:" + *p.Unit
			name = strings.TrimSuffix(strings.TrimSuffix(*p.Unit, ".service"), ".scope")
			subtitle = *p.Unit
			if strings.HasSuffix(*p.Unit, ".scope") {
				category = "apps"
			}
		} else if exeBase != "" {
			key = "exe:" + exeBase
			name = exeBase
			subtitle = p.Command
		} else if p.KernelThread {
			key = "kernel"
			name = "Kernel threads"
			subtitle = "Kernel"
			category = "system"
		} else {
			key = "name:" + p.Name
			name = p.Name
			subtitle = p.Command
		}
		if p.UID == 0 || p.KernelThread {
			category = "system"
		}
		g := groups[key]
		if g == nil {
			g = &ProcessGroup{Key: key, Category: category, Name: fallback(name, p.Name), Subtitle: subtitle, IconKey: iconKey, Unit: p.Unit, LeaderPID: p.PID, Instances: 1, Windows: []WindowRef{}, State: p.State, MemoryKind: "private", Energy: "none", Members: []ProcessInfo{}}
			groups[key] = g
		}
		g.Members = append(g.Members, p)
		g.CPU += p.CPU
		g.Memory += p.Memory
		g.Threads += p.Threads
		g.EnergyScore += p.EnergyScore
		if p.DiskRead != nil {
			if g.DiskRead == nil {
				g.DiskRead = f64p(0)
			}
			*g.DiskRead += *p.DiskRead
		}
		if p.DiskWrite != nil {
			if g.DiskWrite == nil {
				g.DiskWrite = f64p(0)
			}
			*g.DiskWrite += *p.DiskWrite
		}
		if p.GPU != nil {
			if g.GPU == nil {
				g.GPU = f64p(0)
			}
			*g.GPU += *p.GPU
		}
		if p.GPUMemory != nil {
			if g.GPUMemory == nil {
				g.GPUMemory = u64p(0)
			}
			*g.GPUMemory += *p.GPUMemory
		}
		if p.State == "zombie" {
			g.State = "zombie"
		}
	}
	out := make([]ProcessGroup, 0, len(groups))
	for _, g := range groups {
		g.Energy = energyBand(g.EnergyScore)
		sort.Slice(g.Members, func(i, j int) bool { return g.Members[i].CPU > g.Members[j].CPU })
		out = append(out, *g)
	}
	sort.Slice(out, func(i, j int) bool {
		rank := func(c string) int {
			if c == "apps" {
				return 0
			}
			if c == "background" {
				return 1
			}
			return 2
		}
		ri, rj := rank(out[i].Category), rank(out[j].Category)
		if ri != rj {
			return ri < rj
		}
		if out[i].CPU != out[j].CPU {
			return out[i].CPU > out[j].CPU
		}
		return strings.ToLower(out[i].Name) < strings.ToLower(out[j].Name)
	})
	return out, icons
}

type desktopEntry struct{ id, name, icon string }

func scanDesktopEntries() map[string]desktopEntry {
	out := map[string]desktopEntry{}
	dirs := []string{"/run/current-system/sw/share/applications", "/usr/share/applications"}
	if h, e := os.UserHomeDir(); e == nil {
		dirs = append([]string{filepath.Join(h, ".local/share/applications")}, dirs...)
	}
	for _, dir := range dirs {
		entries, _ := os.ReadDir(dir)
		for _, e := range entries {
			if e.IsDir() || !strings.HasSuffix(e.Name(), ".desktop") {
				continue
			}
			b, er := os.ReadFile(filepath.Join(dir, e.Name()))
			if er != nil {
				continue
			}
			var name, execv, icon string
			for _, l := range strings.Split(string(b), "\n") {
				if strings.HasPrefix(l, "Name=") && name == "" {
					name = strings.TrimPrefix(l, "Name=")
				}
				if strings.HasPrefix(l, "Exec=") {
					execv = strings.TrimPrefix(l, "Exec=")
				}
				if strings.HasPrefix(l, "Icon=") {
					icon = strings.TrimPrefix(l, "Icon=")
				}
			}
			if execv == "" {
				continue
			}
			cmd := strings.Fields(execv)
			if len(cmd) == 0 {
				continue
			}
			base := filepath.Base(strings.Trim(cmd[0], "\"'"))
			id := strings.TrimSuffix(e.Name(), ".desktop")
			iconPath := resolveIcon(icon)
			out[base] = desktopEntry{id: id, name: fallback(name, id), icon: iconPath}
		}
	}
	return out
}
func resolveIcon(icon string) string {
	if icon == "" {
		return ""
	}
	if filepath.IsAbs(icon) {
		if _, e := os.Stat(icon); e == nil {
			return icon
		}
	}
	names := []string{icon, icon + ".png", icon + ".svg"}
	roots := []string{"/run/current-system/sw/share/icons/hicolor", "/usr/share/icons/hicolor", "/usr/share/pixmaps"}
	if h, e := os.UserHomeDir(); e == nil {
		roots = append([]string{filepath.Join(h, ".local/share/icons/hicolor")}, roots...)
	}
	for _, root := range roots {
		for _, n := range names {
			matches, _ := filepath.Glob(filepath.Join(root, "*", "apps", n))
			if len(matches) > 0 {
				return matches[len(matches)-1]
			}
			p := filepath.Join(root, n)
			if _, e := os.Stat(p); e == nil {
				return p
			}
		}
	}
	return ""
}

func readMemoryInfo() MemoryInfo {
	v := map[string]uint64{}
	f, e := os.Open("/proc/meminfo")
	if e == nil {
		defer f.Close()
		s := bufio.NewScanner(f)
		for s.Scan() {
			x := strings.Fields(s.Text())
			if len(x) >= 2 {
				n, _ := strconv.ParseUint(x[1], 10, 64)
				v[strings.TrimSuffix(x[0], ":")] = n * 1024
			}
		}
	}
	total, avail := v["MemTotal"], v["MemAvailable"]
	used := safeDelta(total, avail)
	swapUsed := safeDelta(v["SwapTotal"], v["SwapFree"])
	return MemoryInfo{Total: total, Used: used, Available: avail, Free: v["MemFree"], Buffers: v["Buffers"], Cached: v["Cached"] + v["SReclaimable"], Shared: v["Shmem"], Dirty: v["Dirty"], Mapped: v["Mapped"], Committed: v["Committed_AS"], CommitLimit: v["CommitLimit"], SwapTotal: v["SwapTotal"], SwapUsed: swapUsed, SwapCached: v["SwapCached"], Zswap: readZswap(), Pressure: readPressure("/proc/pressure/memory"), Limits: Availability{}}
}
func readZswap() *uint64 {
	b, e := os.ReadFile("/sys/kernel/debug/zswap/stored_pages")
	if e != nil {
		return nil
	}
	n, e := strconv.ParseUint(strings.TrimSpace(string(b)), 10, 64)
	if e != nil {
		return nil
	}
	v := n * uint64(os.Getpagesize())
	return &v
}
func readPressure(path string) *Pressure {
	b, e := os.ReadFile(path)
	if e != nil {
		return nil
	}
	p := &Pressure{}
	for _, l := range strings.Split(string(b), "\n") {
		f := strings.Fields(l)
		if len(f) < 2 {
			continue
		}
		vals := map[string]float64{}
		for _, x := range f[1:] {
			k, v, ok := strings.Cut(x, "=")
			if ok {
				vals[k], _ = strconv.ParseFloat(v, 64)
			}
		}
		if f[0] == "some" {
			p.Some10 = vals["avg10"]
			p.Some60 = vals["avg60"]
			p.Some300 = vals["avg300"]
		}
		if f[0] == "full" {
			p.Full10 = f64p(vals["avg10"])
			p.Full60 = f64p(vals["avg60"])
			p.Full300 = f64p(vals["avg300"])
		}
	}
	return p
}
func readDisks(prev map[string]diskRaw, sampleMS float64, sampled bool) ([]DiskInfo, map[string]diskRaw) {
	current := map[string]diskRaw{}
	mounts := readMounts()
	b, e := os.ReadFile("/proc/diskstats")
	if e != nil {
		return nil, current
	}
	var out []DiskInfo
	for _, l := range strings.Split(string(b), "\n") {
		f := strings.Fields(l)
		if len(f) < 14 {
			continue
		}
		name := f[2]
		if strings.HasPrefix(name, "loop") || strings.HasPrefix(name, "ram") {
			continue
		}
		sys := filepath.Join("/sys/block", name)
		if _, e := os.Stat(sys); e != nil {
			continue
		}
		r := diskRaw{}
		r.readOps, _ = strconv.ParseUint(f[3], 10, 64)
		r.readSectors, _ = strconv.ParseUint(f[5], 10, 64)
		r.writeOps, _ = strconv.ParseUint(f[7], 10, 64)
		r.writeSectors, _ = strconv.ParseUint(f[9], 10, 64)
		r.busyMS, _ = strconv.ParseUint(f[12], 10, 64)
		current[name] = r
		size := readUint(filepath.Join(sys, "size")) * 512
		model := readStringPtr(filepath.Join(sys, "device/model"))
		kind := "hdd"
		if strings.HasPrefix(name, "nvme") {
			kind = "nvme"
		} else if readUint(filepath.Join(sys, "queue/rotational")) == 0 {
			kind = "ssd"
		}
		d := DiskInfo{Name: name, Model: model, Kind: kind, Capacity: size, ReadTotal: r.readSectors * 512, WriteTotal: r.writeSectors * 512, Mounts: mounts[name]}
		if sampled && sampleMS > 0 {
			if o, ok := prev[name]; ok {
				sec := sampleMS / 1000
				d.ReadRate = float64(safeDelta(r.readSectors, o.readSectors)*512) / sec
				d.WriteRate = float64(safeDelta(r.writeSectors, o.writeSectors)*512) / sec
				d.ReadIops = float64(safeDelta(r.readOps, o.readOps)) / sec
				d.WriteIops = float64(safeDelta(r.writeOps, o.writeOps)) / sec
				d.ActivePercent = float64(safeDelta(r.busyMS, o.busyMS)) / sampleMS * 100
				if d.ActivePercent > 100 {
					d.ActivePercent = 100
				}
			}
		}
		out = append(out, d)
	}
	return out, current
}
func readMounts() map[string][]DiskMount {
	out := map[string][]DiskMount{}
	b, e := os.ReadFile("/proc/mounts")
	if e != nil {
		return out
	}
	for _, l := range strings.Split(string(b), "\n") {
		f := strings.Fields(l)
		if len(f) < 3 || !strings.HasPrefix(f[0], "/dev/") {
			continue
		}
		base := filepath.Base(f[0])
		root := base
		for len(root) > 0 && root[len(root)-1] >= '0' && root[len(root)-1] <= '9' {
			root = root[:len(root)-1]
		}
		root = strings.TrimSuffix(root, "p")
		var st syscall.Statfs_t
		if syscall.Statfs(f[1], &st) != nil {
			continue
		}
		total := st.Blocks * uint64(st.Bsize)
		free := st.Bavail * uint64(st.Bsize)
		out[root] = append(out[root], DiskMount{Path: f[1], FS: f[2], Used: safeDelta(total, free), Total: total, System: f[1] == "/"})
	}
	return out
}
func readNetworks(prev map[string]netRaw, sampleMS float64, sampled bool) ([]NetworkInfo, map[string]netRaw) {
	entries, _ := os.ReadDir("/sys/class/net")
	cur := map[string]netRaw{}
	var out []NetworkInfo
	for _, e := range entries {
		name := e.Name()
		base := filepath.Join("/sys/class/net", name)
		r := netRaw{rx: readUint(filepath.Join(base, "statistics/rx_bytes")), tx: readUint(filepath.Join(base, "statistics/tx_bytes"))}
		cur[name] = r
		kind := "ethernet"
		if name == "lo" {
			kind = "loopback"
		} else if _, er := os.Stat(filepath.Join(base, "wireless")); er == nil {
			kind = "wifi"
		} else if strings.HasPrefix(name, "veth") || strings.HasPrefix(name, "br") || strings.HasPrefix(name, "docker") || strings.HasPrefix(name, "vir") || strings.HasPrefix(name, "tun") {
			kind = "virtual"
		}
		n := NetworkInfo{Name: name, Kind: kind, State: readString(filepath.Join(base, "operstate")), RxTotal: r.rx, TxTotal: r.tx, IPv4: []string{}, IPv6: []string{}, MAC: readStringPtr(filepath.Join(base, "address")), MTU: u64ptrIf(readUint(filepath.Join(base, "mtu"))), SpeedMbps: f64ptrIf(float64(readInt(filepath.Join(base, "speed"))))}
		if sampled && sampleMS > 0 {
			if o, ok := prev[name]; ok {
				sec := sampleMS / 1000
				n.RxRate = float64(safeDelta(r.rx, o.rx)) / sec
				n.TxRate = float64(safeDelta(r.tx, o.tx)) / sec
			}
		}
		if kind == "wifi" {
			if ssid, signal, freq := wifiInfo(name); ssid != "" {
				n.SSID = &ssid
				n.Signal = signal
				n.FrequencyMhz = freq
			}
		}
		out = append(out, n)
	}
	return out, cur
}
func wifiInfo(name string) (string, *float64, *float64) {
	cmd := exec.Command("iw", "dev", name, "link")
	b, e := cmd.Output()
	if e != nil {
		return "", nil, nil
	}
	var ssid string
	var sig, freq *float64
	for _, l := range strings.Split(string(b), "\n") {
		t := strings.TrimSpace(l)
		if strings.HasPrefix(t, "SSID:") {
			ssid = strings.TrimSpace(strings.TrimPrefix(t, "SSID:"))
		}
		if strings.HasPrefix(t, "signal:") {
			f := strings.Fields(t)
			if len(f) > 1 {
				v, _ := strconv.ParseFloat(f[1], 64)
				p := 2 * (v + 100)
				sig = &p
			}
		}
		if strings.HasPrefix(t, "freq:") {
			f := strings.Fields(t)
			if len(f) > 1 {
				v, _ := strconv.ParseFloat(f[1], 64)
				freq = &v
			}
		}
	}
	return ssid, sig, freq
}
func readGPUs() []GpuInfo {
	cmd := exec.Command("nvidia-smi", "--query-gpu=index,name,utilization.gpu,utilization.memory,memory.used,memory.total,temperature.gpu,power.draw,power.limit,clocks.gr,clocks.mem,driver_version,pstate", "--format=csv,noheader,nounits")
	b, err := cmd.Output()
	if err != nil {
		return []GpuInfo{}
	}
	var out []GpuInfo
	for _, line := range strings.Split(strings.TrimSpace(string(b)), "\n") {
		fields := strings.Split(line, ",")
		if len(fields) < 13 {
			continue
		}
		for i := range fields {
			fields[i] = strings.TrimSpace(fields[i])
		}
		idx, _ := strconv.ParseUint(fields[0], 10, 32)
		out = append(out, GpuInfo{
			Index: uint32(idx), Name: fields[1], Vendor: "nvidia",
			Usage: parseF64Ptr(fields[2]), MemoryUsage: parseF64Ptr(fields[3]),
			MemoryUsed: parseMiB(fields[4]), MemoryTotal: parseMiB(fields[5]),
			Temperature: parseF64Ptr(fields[6]), Power: parseF64Ptr(fields[7]),
			PowerLimit: parseF64Ptr(fields[8]), ClockMhz: parseF64Ptr(fields[9]),
			MemoryClockMhz: parseF64Ptr(fields[10]), Driver: strp(fields[11]),
			Pstate: strp(fields[12]), Limits: Availability{},
		})
	}
	return out
}
func parseMiB(s string) *uint64 {
	v, e := strconv.ParseFloat(s, 64)
	if e != nil {
		return nil
	}
	n := uint64(v * 1024 * 1024)
	return &n
}
func readEnergy() EnergyInfo {
	e := EnergyInfo{Source: "unknown", Limits: Availability{}}
	entries, _ := filepath.Glob("/sys/class/power_supply/BAT*")
	if len(entries) == 0 {
		e.Source = "ac"
		e.Limits["battery"] = "no battery reading is available"
		return e
	}
	b := entries[0]
	e.Source = "battery"
	e.BatteryPercent = f64ptrIf(float64(readInt(filepath.Join(b, "capacity"))))
	e.BatteryStatus = readStringPtr(filepath.Join(b, "status"))
	now := float64(readUint(filepath.Join(b, "energy_now"))) / 1e6
	full := float64(readUint(filepath.Join(b, "energy_full"))) / 1e6
	design := float64(readUint(filepath.Join(b, "energy_full_design"))) / 1e6
	if now > 0 {
		e.BatteryEnergyNow = &now
	}
	if full > 0 {
		e.BatteryEnergyFull = &full
	}
	if design > 0 {
		e.BatteryEnergyDesign = &design
	}
	power := float64(readUint(filepath.Join(b, "power_now"))) / 1e6
	if power > 0 {
		e.BatteryPower = &power
		if now > 0 && strings.Contains(strings.ToLower(readString(filepath.Join(b, "status"))), "discharg") {
			t := now / power * 3600
			e.TimeToEmptySeconds = &t
		}
	}
	cycles := readUint(filepath.Join(b, "cycle_count"))
	if cycles > 0 {
		e.CycleCount = &cycles
	}
	return e
}
func readThermals() ThermalInfo {
	var sensors []ThermalSensor
	zones, _ := filepath.Glob("/sys/class/thermal/thermal_zone*")
	for _, z := range zones {
		temp := float64(readInt(filepath.Join(z, "temp"))) / 1000
		if temp <= 0 {
			continue
		}
		label := readString(filepath.Join(z, "type"))
		sensors = append(sensors, ThermalSensor{ID: filepath.Base(z), Chip: label, Label: label, Temperature: temp})
	}
	sort.Slice(sensors, func(i, j int) bool { return sensors[i].Temperature > sensors[j].Temperature })
	var hot *ThermalSensor
	if len(sensors) > 0 {
		v := sensors[0]
		hot = &v
	}
	return ThermalInfo{Hotspot: hot, Sensors: sensors}
}
func readSystemInfo(cpu CpuInfo) SystemInfo {
	host, _ := os.Hostname()
	var u syscall.Utsname
	kernel := "Unknown"
	if syscall.Uname(&u) == nil {
		kernel = charsToString(u.Release[:])
	}
	return SystemInfo{Hostname: fallback(host, "Unknown"), OS: readOSName(), Kernel: kernel, CPUModel: cpu.Model, LogicalCPUs: cpu.LogicalCPUs, PhysicalCores: cpu.PhysicalCores}
}
func readOSName() string {
	b, e := os.ReadFile("/etc/os-release")
	if e != nil {
		return "Linux"
	}
	for _, l := range strings.Split(string(b), "\n") {
		if strings.HasPrefix(l, "PRETTY_NAME=") {
			return strings.Trim(strings.TrimPrefix(l, "PRETTY_NAME="), "\"")
		}
	}
	return "Linux"
}
func readCPUModel() string {
	b, e := os.ReadFile("/proc/cpuinfo")
	if e != nil {
		return "Unknown CPU"
	}
	for _, l := range strings.Split(string(b), "\n") {
		if strings.HasPrefix(l, "model name") || strings.HasPrefix(l, "Hardware") {
			if _, v, ok := strings.Cut(l, ":"); ok {
				return strings.TrimSpace(v)
			}
		}
	}
	return "Unknown CPU"
}
func physicalCoreCount() int {
	b, e := os.ReadFile("/proc/cpuinfo")
	if e != nil {
		return runtime.NumCPU()
	}
	cores := map[string]bool{}
	phys, core := "0", ""
	flush := func() {
		if core != "" {
			cores[phys+":"+core] = true
		}
	}
	for _, l := range strings.Split(string(b), "\n") {
		if l == "" {
			flush()
			phys, core = "0", ""
			continue
		}
		k, v, ok := strings.Cut(l, ":")
		if !ok {
			continue
		}
		switch strings.TrimSpace(k) {
		case "physical id":
			phys = strings.TrimSpace(v)
		case "core id":
			core = strings.TrimSpace(v)
		}
	}
	flush()
	if len(cores) == 0 {
		return runtime.NumCPU()
	}
	return len(cores)
}
func socketCount() int {
	b, e := os.ReadFile("/proc/cpuinfo")
	if e != nil {
		return 1
	}
	s := map[string]bool{}
	for _, l := range strings.Split(string(b), "\n") {
		if strings.HasPrefix(l, "physical id") {
			_, v, _ := strings.Cut(l, ":")
			s[strings.TrimSpace(v)] = true
		}
	}
	if len(s) == 0 {
		return 1
	}
	return len(s)
}
func readVirtualization() *string {
	b, e := os.ReadFile("/proc/cpuinfo")
	if e != nil {
		return nil
	}
	if strings.Contains(string(b), " hypervisor ") {
		v := "hardware virtualization"
		return &v
	}
	return nil
}
func readCaches() []CpuCache {
	var out []CpuCache
	paths, _ := filepath.Glob("/sys/devices/system/cpu/cpu0/cache/index*")
	seen := map[string]bool{}
	for _, p := range paths {
		level := readString(filepath.Join(p, "level"))
		typ := readString(filepath.Join(p, "type"))
		size := readString(filepath.Join(p, "size"))
		if level == "" || size == "" {
			continue
		}
		k := level + typ + size
		if seen[k] {
			continue
		}
		seen[k] = true
		out = append(out, CpuCache{Level: "L" + level + " " + typ, Size: size})
	}
	return out
}
func readOpenFiles() (*uint64, *uint64) {
	b, e := os.ReadFile("/proc/sys/fs/file-nr")
	if e != nil {
		return nil, nil
	}
	f := strings.Fields(string(b))
	if len(f) < 3 {
		return nil, nil
	}
	a, _ := strconv.ParseUint(f[0], 10, 64)
	m, _ := strconv.ParseUint(f[2], 10, 64)
	return &a, &m
}
func userName(uid uint32) string {
	u, e := user.LookupId(strconv.FormatUint(uint64(uid), 10))
	if e == nil {
		return u.Username
	}
	return strconv.FormatUint(uint64(uid), 10)
}
func readLinkPtr(path string) *string {
	p, e := os.Readlink(path)
	if e != nil {
		return nil
	}
	v := p
	return &v
}
func readString(path string) string {
	b, e := os.ReadFile(path)
	if e != nil {
		return ""
	}
	return strings.TrimSpace(string(b))
}
func readStringPtr(path string) *string {
	v := readString(path)
	if v == "" {
		return nil
	}
	return &v
}
func readUint(path string) uint64 { v, _ := strconv.ParseUint(readString(path), 10, 64); return v }
func readInt(path string) int64   { v, _ := strconv.ParseInt(readString(path), 10, 64); return v }
func readFloat(path string) *float64 {
	v, e := strconv.ParseFloat(readString(path), 64)
	if e != nil {
		return nil
	}
	return &v
}
func parseF64Ptr(v string) *float64 {
	n, e := strconv.ParseFloat(strings.TrimSpace(v), 64)
	if e != nil {
		return nil
	}
	return &n
}
func f64p(v float64) *float64 { return &v }
func u64p(v uint64) *uint64   { return &v }
func u32p(v uint32) *uint32   { return &v }
func strp(v string) *string {
	if v == "" {
		return nil
	}
	return &v
}
func f64ptrIf(v float64) *float64 {
	if v <= 0 {
		return nil
	}
	return &v
}
func u64ptrIf(v uint64) *uint64 {
	if v == 0 {
		return nil
	}
	return &v
}
func fallback(v, other string) string {
	if strings.TrimSpace(v) == "" {
		return other
	}
	return v
}
func charsToString(chars []int8) string {
	b := make([]byte, 0, len(chars))
	for _, c := range chars {
		if c == 0 {
			break
		}
		b = append(b, byte(c))
	}
	return string(b)
}
func ryokuAccent() string {
	path := ""
	if c := os.Getenv("XDG_CACHE_HOME"); c != "" {
		path = filepath.Join(c, "ryoku/colors.json")
	} else if h, e := os.UserHomeDir(); e == nil {
		path = filepath.Join(h, ".cache/ryoku/colors.json")
	}
	if path == "" {
		return fallbackAccent
	}
	b, e := os.ReadFile(path)
	if e != nil {
		return fallbackAccent
	}
	var p map[string]any
	if json.Unmarshal(b, &p) != nil {
		return fallbackAccent
	}
	v, _ := p["primary"].(string)
	if len(v) == 7 && strings.HasPrefix(v, "#") {
		if _, e := strconv.ParseUint(v[1:], 16, 32); e == nil {
			return v
		}
	}
	return fallbackAccent
}

func processDetail(base ProcessInfo) ProcessInfo {
	pid := base.PID
	if b, e := os.ReadFile(fmt.Sprintf("/proc/%d/smaps_rollup", pid)); e == nil {
		for _, l := range strings.Split(string(b), "\n") {
			f := strings.Fields(l)
			if len(f) < 2 {
				continue
			}
			n, _ := strconv.ParseUint(f[1], 10, 64)
			n *= 1024
			switch strings.TrimSuffix(f[0], ":") {
			case "Pss":
				base.Memory = n
				base.MemoryKind = "pss"
			case "Pss_Anon":
				base.RSSAnon = &n
			case "Pss_File":
				base.RSSFile = &n
			case "Pss_Shmem":
				base.RSSShmem = &n
			case "Swap":
				base.Swap = &n
			}
		}
	}
	if entries, e := os.ReadDir(fmt.Sprintf("/proc/%d/fd", pid)); e == nil {
		n := uint64(len(entries))
		base.FDs = &n
	}
	if b, e := os.ReadFile(fmt.Sprintf("/proc/%d/status", pid)); e == nil {
		var ctx uint64
		for _, l := range strings.Split(string(b), "\n") {
			if strings.HasPrefix(l, "voluntary_ctxt_switches:") || strings.HasPrefix(l, "nonvoluntary_ctxt_switches:") {
				f := strings.Fields(l)
				if len(f) > 1 {
					n, _ := strconv.ParseUint(f[1], 10, 64)
					ctx += n
				}
			}
		}
		base.CtxSwitches = &ctx
	}
	if b, e := os.ReadFile(fmt.Sprintf("/proc/%d/oom_score", pid)); e == nil {
		n, _ := strconv.ParseInt(strings.TrimSpace(string(b)), 10, 64)
		base.OOMScore = &n
	}
	return base
}
