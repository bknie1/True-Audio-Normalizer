// TanControl - a tiny WinForms GUI for the user-mode TAN path.
//
// Wraps tan-live.exe: captures the VB-CABLE virtual device, runs the TAN DSP,
// and plays to the chosen output. On/off toggle, profile + output pickers,
// cable detection with a one-click installer, and a tray icon. Depends only on
// the .NET Framework that ships with Windows (compiled with the in-box csc), so
// the distributable is a single small exe next to tan-live.exe.
//
// This is a fresh, self-contained app (not the tan-tray crate) to avoid
// colliding with the component maintained on the other machine.

using System;
using System.Diagnostics;
using System.Drawing;
using System.IO;
using System.Text.RegularExpressions;
using System.Windows.Forms;

namespace TanControl
{
    public class MainForm : Form
    {
        private readonly string _tanLive;
        private readonly string _setupPs1;
        private Process _engine;
        private readonly ComboBox _profile = new ComboBox();
        private readonly ComboBox _output = new ComboBox();
        private readonly Button _toggle = new Button();
        private readonly Button _installCable = new Button();
        private readonly Label _cableStatus = new Label();
        private readonly Label _status = new Label();
        private readonly NotifyIcon _tray = new NotifyIcon();

        public MainForm()
        {
            _tanLive = LocateFile("tan-live.exe",
                new[] { "", @"..\..\..\target\release\", @"..\..\..\..\target\release\" });
            _setupPs1 = LocateFile("tan-setup.ps1",
                new[] { "", @"..\tan-vad\scripts\", @"scripts\" });

            Text = "TAN - True Audio Normalizer";
            Width = 420; Height = 320;
            FormBorderStyle = FormBorderStyle.FixedSingle;
            MaximizeBox = false;
            StartPosition = FormStartPosition.CenterScreen;

            var title = new Label { Text = "TAN - True Audio Normalizer", Left = 16, Top = 12, Width = 380, Font = new Font(Font.FontFamily, 12, FontStyle.Bold) };
            var hint = new Label { Text = "Set an app's playback device to \"CABLE Input\" to normalize it.", Left = 16, Top = 40, Width = 380, ForeColor = Color.DimGray };

            _cableStatus.Left = 16; _cableStatus.Top = 70; _cableStatus.Width = 250;
            _installCable.Text = "Install VB-CABLE"; _installCable.Left = 270; _installCable.Top = 66; _installCable.Width = 120;
            _installCable.Click += (s, e) => InstallCable();

            var pl = new Label { Text = "Profile:", Left = 16, Top = 108, Width = 60 };
            _profile.Left = 90; _profile.Top = 104; _profile.Width = 150; _profile.DropDownStyle = ComboBoxStyle.DropDownList;
            _profile.Items.AddRange(new object[] { "universal", "movie", "music", "speech", "night", "game" });
            _profile.SelectedItem = "universal";

            var ol = new Label { Text = "Output:", Left = 16, Top = 144, Width = 60 };
            _output.Left = 90; _output.Top = 140; _output.Width = 300; _output.DropDownStyle = ComboBoxStyle.DropDownList;

            _toggle.Text = "Turn TAN On"; _toggle.Left = 16; _toggle.Top = 190; _toggle.Width = 374; _toggle.Height = 44;
            _toggle.Font = new Font(Font.FontFamily, 11, FontStyle.Bold);
            _toggle.Click += (s, e) => Toggle();

            _status.Left = 16; _status.Top = 244; _status.Width = 374; _status.ForeColor = Color.DimGray;

            Controls.AddRange(new Control[] { title, hint, _cableStatus, _installCable, pl, _profile, ol, _output, _toggle, _status });

            _tray.Icon = MakeIcon(Color.Gray);
            _tray.Visible = true;
            _tray.Text = "TAN (off)";
            var menu = new ContextMenu();
            menu.MenuItems.Add("Show", (s, e) => { Show(); WindowState = FormWindowState.Normal; });
            menu.MenuItems.Add("Turn On/Off", (s, e) => Toggle());
            menu.MenuItems.Add("Exit", (s, e) => { StopEngine(); _tray.Visible = false; Application.Exit(); });
            _tray.ContextMenu = menu;
            _tray.DoubleClick += (s, e) => { Show(); WindowState = FormWindowState.Normal; };

            FormClosing += (s, e) => StopEngine();
            Resize += (s, e) => { if (WindowState == FormWindowState.Minimized) Hide(); };

            RefreshCable();
            PopulateOutputs();
            if (_tanLive == null) { _status.Text = "tan-live.exe not found next to this app."; _toggle.Enabled = false; }
        }

        private static string LocateFile(string name, string[] relDirs)
        {
            string baseDir = AppDomain.CurrentDomain.BaseDirectory;
            foreach (var rel in relDirs)
            {
                try { var p = Path.GetFullPath(Path.Combine(baseDir, rel, name)); if (File.Exists(p)) return p; } catch { }
            }
            return null;
        }

        private Icon MakeIcon(Color c)
        {
            using (var bmp = new Bitmap(16, 16))
            using (var g = Graphics.FromImage(bmp))
            {
                g.SmoothingMode = System.Drawing.Drawing2D.SmoothingMode.AntiAlias;
                g.Clear(Color.Transparent);
                using (var br = new SolidBrush(c)) g.FillEllipse(br, 2, 2, 12, 12);
                return Icon.FromHandle(bmp.GetHicon());
            }
        }

        private string RunTanLive(string args)
        {
            var psi = new ProcessStartInfo(_tanLive, args)
            { UseShellExecute = false, RedirectStandardOutput = true, RedirectStandardError = true, CreateNoWindow = true };
            using (var p = Process.Start(psi))
            {
                string outp = p.StandardOutput.ReadToEnd();
                p.WaitForExit(5000);
                return outp;
            }
        }

        private void PopulateOutputs()
        {
            _output.Items.Clear();
            _output.Items.Add("System default");
            _output.SelectedIndex = 0;
            if (_tanLive == null) return;
            try
            {
                string list = RunTanLive("--list-devices");
                bool inOutputs = false;
                foreach (var line in list.Split('\n'))
                {
                    if (line.IndexOf("output devices", StringComparison.OrdinalIgnoreCase) >= 0) { inOutputs = true; continue; }
                    if (inOutputs)
                    {
                        var m = Regex.Match(line, @"^\s*\[\d+\]\s+(.+?)\s*$");
                        if (m.Success) { var name = m.Groups[1].Value; if (name.IndexOf("CABLE", StringComparison.OrdinalIgnoreCase) < 0) _output.Items.Add(name); }
                        else if (line.Trim().Length == 0) break;
                    }
                }
            }
            catch { }
        }

        private bool CablePresent()
        {
            if (_tanLive == null) return false;
            try { return RunTanLive("--list-devices").IndexOf("CABLE", StringComparison.OrdinalIgnoreCase) >= 0; }
            catch { return false; }
        }

        private void RefreshCable()
        {
            bool present = CablePresent();
            _cableStatus.Text = present ? "Virtual cable: installed" : "Virtual cable: NOT installed";
            _cableStatus.ForeColor = present ? Color.Green : Color.Firebrick;
            _installCable.Enabled = !present;
            _toggle.Enabled = present && _tanLive != null;
        }

        private void InstallCable()
        {
            if (_setupPs1 == null) { MessageBox.Show("tan-setup.ps1 not found; install VB-CABLE from vb-audio.com/Cable/."); return; }
            var psi = new ProcessStartInfo("powershell.exe",
                "-NoProfile -ExecutionPolicy Bypass -File \"" + _setupPs1 + "\" -InstallCable")
            { UseShellExecute = true };
            Process.Start(psi);
            MessageBox.Show("The VB-CABLE installer will open. Click \"Install Driver\", accept the prompt, reboot if asked, then click OK here.", "Install VB-CABLE");
            RefreshCable();
        }

        private void Toggle()
        {
            if (_engine != null && !_engine.HasExited) { StopEngine(); return; }
            if (_tanLive == null || !CablePresent()) { RefreshCable(); return; }
            string args = "--loopback-from \"CABLE Output\" --profile " + _profile.SelectedItem;
            if (_output.SelectedIndex > 0) args += " --output \"" + _output.SelectedItem + "\"";
            try
            {
                var psi = new ProcessStartInfo(_tanLive, args) { UseShellExecute = false, CreateNoWindow = true };
                _engine = Process.Start(psi);
                _toggle.Text = "Turn TAN Off";
                _status.Text = "TAN is ON (" + _profile.SelectedItem + ")";
                _tray.Icon = MakeIcon(Color.FromArgb(60, 200, 90));
                _tray.Text = "TAN (on)";
            }
            catch (Exception ex) { MessageBox.Show("Could not start TAN: " + ex.Message); }
        }

        private void StopEngine()
        {
            try { if (_engine != null && !_engine.HasExited) _engine.Kill(); } catch { }
            _engine = null;
            _toggle.Text = "Turn TAN On";
            _status.Text = "TAN is off (raw audio)";
            _tray.Icon = MakeIcon(Color.Gray);
            _tray.Text = "TAN (off)";
        }

        [STAThread]
        public static void Main()
        {
            Application.EnableVisualStyles();
            Application.SetCompatibleTextRenderingDefault(false);
            Application.Run(new MainForm());
        }
    }
}
