package main

import (
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"testing"
)

func writeCard(t *testing.T, root, card, vendor string, files map[string]string) {
	t.Helper()
	dev := filepath.Join(root, card, "device")
	if err := os.MkdirAll(dev, 0o755); err != nil {
		t.Fatalf("lay out %s: %v", card, err)
	}
	files["vendor"] = vendor
	for name, body := range files {
		at := filepath.Join(dev, name)
		if err := os.MkdirAll(filepath.Dir(at), 0o755); err != nil {
			t.Fatalf("lay out %s: %v", name, err)
		}
		if err := os.WriteFile(at, []byte(body+"\n"), 0o644); err != nil {
			t.Fatalf("write %s: %v", name, err)
		}
	}
}

func TestAmdCardIsReadFromSysfs(t *testing.T) {
	root := t.TempDir()
	writeCard(t, root, "card1", "0x1002", map[string]string{
		"product_name":                      "Radeon RX 9070",
		"gpu_busy_percent":                  "37",
		"mem_info_vram_used":                "6540000000",
		"mem_info_vram_total":               "17095983104",
		"power_dpm_force_performance_level": "auto",
		"pp_dpm_sclk":                       "0: 500Mhz\n1: 1579Mhz *\n2: 2400Mhz",
		"pp_dpm_mclk":                       "0: 96Mhz\n1: 1258Mhz *",
		"hwmon/hwmon3/temp1_input":          "42000",
		"hwmon/hwmon3/power1_average":       "59000000",
		"hwmon/hwmon3/power1_cap":           "304000000",
	})

	gpus := readDrmGPUsIn(root)
	if len(gpus) != 1 {
		t.Fatalf("read %d cards, want the one that was laid out", len(gpus))
	}
	g := gpus[0]
	if g.Vendor != "amd" || g.Name != "Radeon RX 9070" {
		t.Errorf("read %q from %q, want the amd card by its own name", g.Name, g.Vendor)
	}
	if g.Usage == nil {
		t.Errorf("no usage read at all")
	} else if *g.Usage != 37 {
		t.Errorf("usage %v, want 37", *g.Usage)
	}
	if g.Temperature == nil {
		t.Errorf("no temperature read at all")
	} else if *g.Temperature != 42 {
		t.Errorf("temperature %v, want 42 degrees from 42000 millidegrees", *g.Temperature)
	}
	if g.Power == nil {
		t.Errorf("no power read at all")
	} else if *g.Power != 59 {
		t.Errorf("power %v, want 59 watts from 59000000 microwatts", *g.Power)
	}
	if g.PowerLimit == nil {
		t.Errorf("no power limit read at all")
	} else if *g.PowerLimit != 304 {
		t.Errorf("power limit %v, want 304 watts", *g.PowerLimit)
	}
	if g.ClockMhz == nil {
		t.Error("no clock read at all")
	} else if *g.ClockMhz != 1579 {
		t.Errorf("clock %v, want the starred 1579 and not 500 or 2400", *g.ClockMhz)
	}
	if g.MemoryClockMhz == nil {
		t.Error("no memory clock read at all")
	} else if *g.MemoryClockMhz != 1258 {
		t.Errorf("memory clock %v, want the starred 1258", *g.MemoryClockMhz)
	}
	if g.MemoryUsed == nil || g.MemoryTotal == nil || g.MemoryUsage == nil {
		t.Fatalf("memory went unread: used=%v total=%v percent=%v", g.MemoryUsed, g.MemoryTotal, g.MemoryUsage)
	}
	if want := float64(6540000000) / float64(17095983104) * 100; *g.MemoryUsage != want {
		t.Errorf("memory usage %v%%, want %v%%", *g.MemoryUsage, want)
	}
}

func TestIntelCardSaysWhatItCannotAnswer(t *testing.T) {
	root := t.TempDir()
	writeCard(t, root, "card0", "0x8086", map[string]string{
		"hwmon/hwmon1/temp1_input": "51000",
	})

	gpus := readDrmGPUsIn(root)
	if len(gpus) != 1 {
		t.Fatalf("read %d cards, want one", len(gpus))
	}
	g := gpus[0]
	if g.Vendor != "intel" {
		t.Errorf("vendor %q, want intel", g.Vendor)
	}
	if g.Usage != nil {
		t.Errorf("usage %v, want nothing: i915 publishes no gpu_busy_percent", *g.Usage)
	}
	if _, said := g.Limits["gpuUsage"]; !said {
		t.Error("no utilisation and no reason given for it")
	}
	if g.MemoryTotal != nil {
		t.Errorf("memory total %v, want nothing: this card shares system memory", *g.MemoryTotal)
	}
	if _, said := g.Limits["gpuMemory"]; !said {
		t.Error("no memory figure and no reason given for it")
	}
	if g.Temperature == nil || *g.Temperature != 51 {
		t.Errorf("temperature %v, want 51 from the hwmon that is there", g.Temperature)
	}
}

func TestNvidiaIsLeftToItsOwnTool(t *testing.T) {
	root := t.TempDir()
	writeCard(t, root, "card0", "0x10de", map[string]string{"product_name": "GeForce RTX 3050"})
	writeCard(t, root, "card1", "0x1002", map[string]string{"gpu_busy_percent": "5"})

	gpus := readDrmGPUsIn(root)
	if len(gpus) != 1 {
		t.Fatalf("read %d cards, want only the amd one", len(gpus))
	}
	if gpus[0].Vendor != "amd" {
		t.Errorf("read the %q card, want amd", gpus[0].Vendor)
	}
}

func TestNothingReadableIsNotListed(t *testing.T) {
	root := t.TempDir()
	if err := os.MkdirAll(filepath.Join(root, "card0", "device"), 0o755); err != nil {
		t.Fatal(err)
	}
	if gpus := readDrmGPUsIn(root); len(gpus) != 0 {
		t.Fatalf("read %d cards from a directory with no vendor, want none", len(gpus))
	}
	if gpus := readDrmGPUsIn(filepath.Join(root, "nowhere")); len(gpus) != 0 {
		t.Fatalf("read %d cards from a path that is not there, want none", len(gpus))
	}
}

func TestTheCardInThisMachineAnswers(t *testing.T) {
	gpus := readDrmGPUs()
	if len(gpus) == 0 {
		t.Skip("no AMD or Intel card in this machine")
	}
	for _, g := range gpus {
		t.Logf("%s (%s/%s): usage=%v mem=%v/%v temp=%v power=%v/%v clock=%v/%v",
			g.Name, g.Vendor, deref(g.Driver), derefF(g.Usage), derefU(g.MemoryUsed), derefU(g.MemoryTotal),
			derefF(g.Temperature), derefF(g.Power), derefF(g.PowerLimit), derefF(g.ClockMhz), derefF(g.MemoryClockMhz))
		if g.Driver == nil || *g.Driver == "" {
			t.Errorf("%s was listed without the module that drives it", g.Name)
		}
	}
}

func deref(s *string) string {
	if s == nil {
		return "-"
	}
	return *s
}

func derefF(f *float64) string {
	if f == nil {
		return "-"
	}
	return strconv.FormatFloat(*f, 'f', -1, 64)
}

func derefU(u *uint64) string {
	if u == nil {
		return "-"
	}
	return strconv.FormatUint(*u, 10)
}

const pciIDs = `# a comment
1002  Advanced Micro Devices, Inc. [AMD/ATI]
	7550  Navi 48 [Radeon RX 9070/9070 XT]
		1002 0e3b  Radeon RX 9070
	743f  Navi 24 [Radeon RX 6400/6500]
8086  Intel Corporation
	a7a0  Raptor Lake-P [Iris Xe Graphics]
	1234  Plain Named Part
`

func TestTheCardIsNamedFromThePCIDatabase(t *testing.T) {
	for _, c := range []struct{ vendor, device, want string }{
		{"0x1002", "0x7550", "Radeon RX 9070/9070 XT"},
		{"0x1002", "0x743f", "Radeon RX 6400/6500"},
		{"0x8086", "0xa7a0", "Iris Xe Graphics"},
		{"0x8086", "0x1234", "Plain Named Part"},
		{"0x8086", "0x7550", ""},
		{"0x9999", "0x7550", ""},
	} {
		got := scanPCIIDs(strings.NewReader(pciIDs), trimHex(c.vendor), trimHex(c.device))
		if got != c.want {
			t.Errorf("%s:%s read as %q, wanted %q", c.vendor, c.device, got, c.want)
		}
	}
}
