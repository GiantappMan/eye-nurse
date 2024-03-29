﻿using Common.Apps.Services;
using Common.WinAPI;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using EyeNurse.Models.UserConfigs;
using EyeNurse.Services;
using EyeNurse.Views;
using Win32 = Microsoft.Win32;
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Threading;
using System.Threading.Tasks;
using System.Windows;
using UserConfigs = EyeNurse.Models.UserConfigs;

namespace EyeNurse.ViewModels
{
    public class EyeNurseViewModel : ObservableObject
    {
        #region fields
        //UI
        private readonly CountdownWindow _countdown;
        private readonly List<Window> _openWindows = new();
        //其他
        private bool _isLocking = false;
        private GlobalTimerService? _timerService;
        private readonly SemaphoreSlim _semaphoreSlim = new(1, 1);
        private readonly List<PInvoke.User32.MONITORINFO> _screens;
        private readonly EyeNurseService _eyeNurseService;
        private DateTime? _lockTime;
        private Setting? _setting;
        #endregion

        #region construct

#pragma warning disable CS8618   //xaml用
        public EyeNurseViewModel() { }
#pragma warning restore CS8618 
        public EyeNurseViewModel(EyeNurseService service)
        {
            //锁屏检测
            Win32.SystemEvents.SessionSwitch += SystemEvents_SessionSwitch;
            _eyeNurseService = service;
            _eyeNurseService.SettingChanged += SettingChanged;
            _screens = User32Ex.GetMonitorInfos()!;

            _countdown = new CountdownWindow()
            {
                DataContext = this
            };
            CloseCommand = new RelayCommand(CloseLockScreen);
            Init();
        }

        #endregion

        #region private
        private void SystemEvents_SessionSwitch(object sender, Win32.SessionSwitchEventArgs e)
        {
            switch (e.Reason)
            {
                case Win32.SessionSwitchReason.SessionUnlock:
                    Resume();
                    if (_lockTime != null && _setting != null)
                    {
                        if (DateTime.Now - _lockTime > _setting.ResetTimeout)
                        {
                            if (_isResting)
                            {
                                CloseLockScreen();
                            }
                            _lockTime = null;
                            Reset();
                        }
                    }
                    break;
                case Win32.SessionSwitchReason.SessionLock:
                    _lockTime = DateTime.Now;
                    Pause();
                    break;
            }
        }
        private void Init()
        {
            _setting = _eyeNurseService.LoadUserConfig<Setting>();
            MainInterval = _setting.RestInterval;
            MainCountdown = _setting.RestInterval;//UI立即显示时间

            //释放
            if (_timerService != null)
            {
                _timerService.Elapsed -= TimerService_Elapsed;
                _timerService.Trigger -= TimerService_Trigger;
                _timerService.Stop();
            }

            _timerService = new GlobalTimerService(_setting.RestInterval);
            _timerService.Elapsed += TimerService_Elapsed;
            _timerService.Trigger += TimerService_Trigger;

            _timerService.Start();
        }
        private void SettingChanged(object? sender, Setting e)
        {
            Init();
        }
        private void CloseLockScreen()
        {
            _isLocking = false;
            //关闭所有_openWindows
            foreach (var item in _openWindows)
            {
                item.Close();
            }
            _openWindows.Clear();
        }
        private void Window_Loaded(object sender, RoutedEventArgs e)
        {
            Window window = (Window)sender;
            window.Loaded -= Window_Loaded;
            window.WindowState = WindowState.Maximized;
        }
        private void TimerService_Elapsed(object? sender, TimerElapsedEventArgs e)
        {
            MainCountdown = e.RemainingTime;
        }
        private void TimerService_Trigger(object? sender, TimerTriggerEventArgs e)
        {
            _countdown.Dispatcher.BeginInvoke(RestNow);
        }
        private static void OpenBrowser(string? url)
        {
            try
            {
                if (url == null)
                    return;
                Process.Start(new ProcessStartInfo(url) { UseShellExecute = true });
            }
            catch (Exception ex)
            {
                System.Windows.MessageBox.Show(ex.ToString());
            }
        }
        #endregion

        #region public
        public async void ShowCountdownWindow()
        {
            var uiConfig = await _eyeNurseService.LoadUserConfigAsync<UI>();
            TopMost = uiConfig.TopMost;
            _ = _countdown.Dispatcher.BeginInvoke(() =>
             {
                 _countdown.Left = uiConfig.CoutdownWindowLeft;
                 _countdown.Top = uiConfig.CoutdownWindowTop;
                 _countdown.Show();
                 if (!_countdown.IsActive)
                     _countdown.Activate();
             });
        }
        public async void SaveCountdownWindowPosition(double left, double top)
        {
            var uiConfig = await _eyeNurseService.LoadUserConfigAsync<UI>();
            uiConfig.CoutdownWindowLeft = left;
            uiConfig.CoutdownWindowTop = top;
            await _eyeNurseService.SaveUserConfigAsync(uiConfig);
        }
        public async void SaveTopMost(bool topMost)
        {
            var uiConfig = await _eyeNurseService.LoadUserConfigAsync<UI>();
            uiConfig.TopMost = topMost;
            await _eyeNurseService.SaveUserConfigAsync(uiConfig);
        }
        /// <summary>
        /// 立即休息
        /// </summary>
        public async void RestNow()
        {
            if (IsResting)
                return;
            IsResting = true;
            _timerService?.Stop();
            var settingConfig = _eyeNurseService.LoadUserConfig<UserConfigs.Setting>();
            foreach (var item in _screens)
            {
                LockScreenWindow window = new();
                window.Loaded += Window_Loaded;
                window.DataContext = this;
                window.Left = item.rcWork.left;
                window.Top = item.rcWork.top;
                window.Height = item.rcWork.bottom - item.rcWork.top;
                window.Width = item.rcWork.right - item.rcWork.left;
                Debug.WriteLine($"left:{window.Left},top:{window.Top},height:{window.Height},width:{window.Width}");
                _openWindows.Add(window);
                window.Show();
            }
            await StartLockScreenCountdown(settingConfig.RestDuration);
            CloseLockScreen();
            Reset();
            _timerService?.Start();
            IsResting = false;
        }
        public async Task StartLockScreenCountdown(TimeSpan countDown)
        {
            if (_isLocking)
                return;
            try
            {
                await _semaphoreSlim.WaitAsync();
                _isLocking = true;
                LockScreenCountdown = countDown;
                while (_isLocking)
                {
                    await Task.Delay(1000);
                    LockScreenCountdown = LockScreenCountdown?.Add(TimeSpan.FromSeconds(-1));
                    if (LockScreenCountdown?.TotalSeconds < 0)
                        _isLocking = false;
                }
            }
            catch (Exception)
            {

            }
            finally
            {
                _semaphoreSlim.Release();
            }
        }
        internal void Reset()
        {
            if (IsPaused)
                Resume();
            _timerService?.Reset();
        }
        internal void Resume()
        {
            if (_timerService != null && _timerService.Resume())
                IsPaused = false;
            PausedTime = null;
        }
        internal async void Pause()
        {
            if (IsPaused)
                return;

            if (_timerService != null && _timerService.Pause())
                IsPaused = true;

            while (IsPaused)
            {
                if (PausedTime == null)
                    PausedTime = new();
                PausedTime = PausedTime.Value + TimeSpan.FromSeconds(1);
                await Task.Delay(1000);
            }
        }
#pragma warning disable CA1822 // 将成员标记为 static
        internal void ShowSetting()
#pragma warning restore CA1822 // 将成员标记为 static
        {
            var vm = IocService.GetService<MainViewModel>()!;
            vm.ShowWindow();
        }
#pragma warning disable CA1822 // 将成员标记为 static
        internal void About()
#pragma warning restore CA1822 // 将成员标记为 static
        {
            OpenBrowser("https://www.giantapp.cn/post/products/eyenurse2");
        }
        #endregion

        #region commands
        public RelayCommand CloseCommand { get; }
        #endregion

        #region properties
        //锁定倒计时
        TimeSpan? _lockScreenCountdown;
        public TimeSpan? LockScreenCountdown { get => _lockScreenCountdown; set => SetProperty(ref _lockScreenCountdown, value); }

        //切换间隔        
        TimeSpan? _mainInterval;
        public TimeSpan? MainInterval { get => _mainInterval; set => SetProperty(ref _mainInterval, value); }

        //主窗口倒计时
        TimeSpan? _mainCountdown;
        public TimeSpan? MainCountdown { get => _mainCountdown; set => SetProperty(ref _mainCountdown, value); }

        bool _isPaused;
        public bool IsPaused { get => _isPaused; set => SetProperty(ref _isPaused, value); }

        TimeSpan? _pausedTime;
        public TimeSpan? PausedTime { get => _pausedTime; set => SetProperty(ref _pausedTime, value); }

        //bool _isDelaying;
        //public bool IsDelaying { get => _isDelaying; set => SetProperty(ref _isDelaying, value); }

        bool _isResting;
        public bool IsResting { get => _isResting; set => SetProperty(ref _isResting, value); }

        //是否置顶
        bool _topMost;
        public bool TopMost
        {
            get => _topMost;
            set => SetProperty(ref _topMost, value);
        }

        #endregion
    }
}
