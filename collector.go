package main

import (
	"bufio"
	"encoding/json"
	"fmt"
	"os"
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

type ProcessInfo struct {
	PID       int     `json:"pid"`
	ParentPID *int    `json:"parentPid"`
	Name      string  `json:"name"`
	Command   string  `json:"command"`
	Status    string  `json:"status"`
	CPU       float64 `json:"cpu"`
	Memory    uint64  `json:"memory"`
}

type SystemInfo struct {
	Hostname      string `json:"hostname"`
	OS            string `json:"os"`
	Kernel        string `json:"kernel"`
	CPUModel      string `json:"cpuModel"`
	LogicalCPUs   int    `json:"logicalCpus"`
	PhysicalCores int    `json:"physicalCores"`
}

type Snapshot struct {
	TimestampMS  int64         `json:"timestampMs"`
	Accent       string        `json:"accent"`
	CPU          float64       `json:"cpu"`
	TotalMemory  uint64        `json:"totalMemory"`
	UsedMemory   uint64        `json:"usedMemory"`
	TotalSwap    uint64        `json:"totalSwap"`
	UsedSwap     uint64        `json:"usedSwap"`
	ProcessCount int           `json:"processCount"`
	Processes    []ProcessInfo `json:"processes"`
	System       SystemInfo    `json:"system"`
}

type cpuSample struct {
	total uint64
	idle  uint64
}

type Collector struct {
	mu          sync.Mutex
	lastCPU     cpuSample
	lastProcCPU map[int]uint64
	hasSample   bool
}

func NewCollector() *Collector {
	return &Collector{lastProcCPU: make(map[int]uint64)}
}

func (c *Collector) Snapshot() Snapshot {
	c.mu.Lock()
	defer c.mu.Unlock()

	cpuNow := readCPUSample()
	mem := readMemoryInfo()
	processes, procTicks := readProcesses(c.lastProcCPU, cpuNow.total-c.lastCPU.total, c.hasSample)

	globalCPU := 0.0
	if c.hasSample {
		deltaTotal := cpuNow.total - c.lastCPU.total
		deltaIdle := cpuNow.idle - c.lastCPU.idle
		if deltaTotal > 0 {
			globalCPU = float64(deltaTotal-deltaIdle) / float64(deltaTotal) * 100
		}
	}

	c.lastCPU = cpuNow
	c.lastProcCPU = procTicks
	c.hasSample = true

	return Snapshot{
		TimestampMS:  time.Now().UnixMilli(),
		Accent:       ryokuAccent(),
		CPU:          globalCPU,
		TotalMemory:  mem.total,
		UsedMemory:   mem.used,
		TotalSwap:    mem.swapTotal,
		UsedSwap:     mem.swapUsed,
		ProcessCount: len(processes),
		Processes:    processes,
		System:       readSystemInfo(),
	}
}

type memoryInfo struct {
	total, used, swapTotal, swapUsed uint64
}

func readMemoryInfo() memoryInfo {
	values := map[string]uint64{}
	file, err := os.Open("/proc/meminfo")
	if err != nil {
		return memoryInfo{}
	}
	defer file.Close()

	scanner := bufio.NewScanner(file)
	for scanner.Scan() {
		fields := strings.Fields(scanner.Text())
		if len(fields) < 2 {
			continue
		}
		key := strings.TrimSuffix(fields[0], ":")
		value, err := strconv.ParseUint(fields[1], 10, 64)
		if err == nil {
			values[key] = value * 1024
		}
	}

	total := values["MemTotal"]
	available := values["MemAvailable"]
	used := uint64(0)
	if total >= available {
		used = total - available
	}
	swapTotal := values["SwapTotal"]
	swapFree := values["SwapFree"]
	swapUsed := uint64(0)
	if swapTotal >= swapFree {
		swapUsed = swapTotal - swapFree
	}
	return memoryInfo{total: total, used: used, swapTotal: swapTotal, swapUsed: swapUsed}
}

func readCPUSample() cpuSample {
	file, err := os.Open("/proc/stat")
	if err != nil {
		return cpuSample{}
	}
	defer file.Close()

	scanner := bufio.NewScanner(file)
	if !scanner.Scan() {
		return cpuSample{}
	}
	fields := strings.Fields(scanner.Text())
	if len(fields) < 5 || fields[0] != "cpu" {
		return cpuSample{}
	}

	var nums []uint64
	for _, field := range fields[1:] {
		value, err := strconv.ParseUint(field, 10, 64)
		if err != nil {
			value = 0
		}
		nums = append(nums, value)
	}
	var total uint64
	for _, value := range nums {
		total += value
	}
	idle := nums[3]
	if len(nums) > 4 {
		idle += nums[4]
	}
	return cpuSample{total: total, idle: idle}
}

func readProcesses(previous map[int]uint64, deltaTotal uint64, hasSample bool) ([]ProcessInfo, map[int]uint64) {
	entries, err := os.ReadDir("/proc")
	if err != nil {
		return nil, map[int]uint64{}
	}

	current := make(map[int]uint64)
	processes := make([]ProcessInfo, 0, len(entries))
	logicalCPUs := runtime.NumCPU()

	for _, entry := range entries {
		if !entry.IsDir() {
			continue
		}
		pid, err := strconv.Atoi(entry.Name())
		if err != nil {
			continue
		}

		statRaw, err := os.ReadFile(filepath.Join("/proc", entry.Name(), "stat"))
		if err != nil {
			continue
		}
		name, parentPID, state, ticks, ok := parseProcStat(string(statRaw))
		if !ok {
			continue
		}
		current[pid] = ticks

		cpuPercent := 0.0
		if hasSample && deltaTotal > 0 {
			if before, found := previous[pid]; found && ticks >= before {
				cpuPercent = float64(ticks-before) / float64(deltaTotal) * float64(logicalCPUs) * 100
			}
		}

		memory := readProcessRSS(pid)
		command := readCmdline(pid)
		parent := parentPID
		processes = append(processes, ProcessInfo{
			PID:       pid,
			ParentPID: &parent,
			Name:      name,
			Command:   command,
			Status:    processStateName(state),
			CPU:       cpuPercent,
			Memory:    memory,
		})
	}

	sort.Slice(processes, func(i, j int) bool {
		if processes[i].CPU == processes[j].CPU {
			return processes[i].Memory > processes[j].Memory
		}
		return processes[i].CPU > processes[j].CPU
	})
	return processes, current
}

func parseProcStat(raw string) (name string, parentPID int, state byte, ticks uint64, ok bool) {
	open := strings.IndexByte(raw, '(')
	close := strings.LastIndex(raw, ") ")
	if open < 0 || close < 0 || close+2 >= len(raw) {
		return "", 0, 0, 0, false
	}
	name = raw[open+1 : close]
	rest := strings.Fields(raw[close+2:])
	if len(rest) < 13 {
		return "", 0, 0, 0, false
	}
	if len(rest[0]) == 0 {
		return "", 0, 0, 0, false
	}
	state = rest[0][0]
	parentPID, err := strconv.Atoi(rest[1])
	if err != nil {
		return "", 0, 0, 0, false
	}
	utime, err1 := strconv.ParseUint(rest[11], 10, 64)
	stime, err2 := strconv.ParseUint(rest[12], 10, 64)
	if err1 != nil || err2 != nil {
		return "", 0, 0, 0, false
	}
	return name, parentPID, state, utime + stime, true
}

func readProcessRSS(pid int) uint64 {
	file, err := os.Open(fmt.Sprintf("/proc/%d/status", pid))
	if err != nil {
		return 0
	}
	defer file.Close()
	scanner := bufio.NewScanner(file)
	for scanner.Scan() {
		line := scanner.Text()
		if strings.HasPrefix(line, "VmRSS:") {
			fields := strings.Fields(line)
			if len(fields) >= 2 {
				value, _ := strconv.ParseUint(fields[1], 10, 64)
				return value * 1024
			}
		}
	}
	return 0
}

func readCmdline(pid int) string {
	raw, err := os.ReadFile(fmt.Sprintf("/proc/%d/cmdline", pid))
	if err != nil || len(raw) == 0 {
		return ""
	}
	parts := strings.Split(strings.TrimRight(string(raw), "\x00"), "\x00")
	return strings.Join(parts, " ")
}

func processStateName(state byte) string {
	switch state {
	case 'R':
		return "Run"
	case 'S':
		return "Sleep"
	case 'D':
		return "UninterruptibleDiskSleep"
	case 'Z':
		return "Zombie"
	case 'T', 't':
		return "Stop"
	case 'X', 'x':
		return "Dead"
	case 'I':
		return "Idle"
	default:
		return string(state)
	}
}

func readSystemInfo() SystemInfo {
	hostname, _ := os.Hostname()
	var uname syscall.Utsname
	kernel := "Unknown"
	if syscall.Uname(&uname) == nil {
		kernel = charsToString(uname.Release[:])
	}
	return SystemInfo{
		Hostname:      fallback(hostname, "Unknown"),
		OS:            readOSName(),
		Kernel:        kernel,
		CPUModel:      readCPUModel(),
		LogicalCPUs:   runtime.NumCPU(),
		PhysicalCores: physicalCoreCount(),
	}
}

func charsToString(chars []int8) string {
	bytes := make([]byte, 0, len(chars))
	for _, char := range chars {
		if char == 0 {
			break
		}
		bytes = append(bytes, byte(char))
	}
	return string(bytes)
}

func readOSName() string {
	raw, err := os.ReadFile("/etc/os-release")
	if err != nil {
		return "Linux"
	}
	for _, line := range strings.Split(string(raw), "\n") {
		if strings.HasPrefix(line, "PRETTY_NAME=") {
			return strings.Trim(strings.TrimPrefix(line, "PRETTY_NAME="), "\"")
		}
	}
	return "Linux"
}

func readCPUModel() string {
	file, err := os.Open("/proc/cpuinfo")
	if err != nil {
		return "Unknown CPU"
	}
	defer file.Close()
	scanner := bufio.NewScanner(file)
	for scanner.Scan() {
		line := scanner.Text()
		if strings.HasPrefix(line, "model name") || strings.HasPrefix(line, "Hardware") {
			if _, value, ok := strings.Cut(line, ":"); ok {
				return strings.TrimSpace(value)
			}
		}
	}
	return "Unknown CPU"
}

func physicalCoreCount() int {
	file, err := os.Open("/proc/cpuinfo")
	if err != nil {
		return 0
	}
	defer file.Close()
	cores := map[string]struct{}{}
	physicalID, coreID := "0", ""
	scanner := bufio.NewScanner(file)
	flush := func() {
		if coreID != "" {
			cores[physicalID+":"+coreID] = struct{}{}
		}
	}
	for scanner.Scan() {
		line := scanner.Text()
		if line == "" {
			flush()
			physicalID, coreID = "0", ""
			continue
		}
		if key, value, ok := strings.Cut(line, ":"); ok {
			switch strings.TrimSpace(key) {
			case "physical id":
				physicalID = strings.TrimSpace(value)
			case "core id":
				coreID = strings.TrimSpace(value)
			}
		}
	}
	flush()
	if len(cores) == 0 {
		return runtime.NumCPU()
	}
	return len(cores)
}

func ryokuAccent() string {
	path := ""
	if cache := os.Getenv("XDG_CACHE_HOME"); cache != "" {
		path = filepath.Join(cache, "ryoku", "colors.json")
	} else if home, err := os.UserHomeDir(); err == nil {
		path = filepath.Join(home, ".cache", "ryoku", "colors.json")
	}
	if path == "" {
		return fallbackAccent
	}
	raw, err := os.ReadFile(path)
	if err != nil {
		return fallbackAccent
	}
	var palette map[string]any
	if json.Unmarshal(raw, &palette) != nil {
		return fallbackAccent
	}
	primary, _ := palette["primary"].(string)
	if validHexColour(primary) {
		return primary
	}
	return fallbackAccent
}

func validHexColour(value string) bool {
	if len(value) != 7 || value[0] != '#' {
		return false
	}
	_, err := strconv.ParseUint(value[1:], 16, 32)
	return err == nil
}

func fallback(value, other string) string {
	if strings.TrimSpace(value) == "" {
		return other
	}
	return value
}
