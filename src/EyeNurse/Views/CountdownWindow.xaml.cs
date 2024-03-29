﻿using EyeNurse.ViewModels;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Data;
using System.Windows.Documents;
using System.Windows.Input;
using System.Windows.Interop;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using System.Windows.Shapes;

namespace EyeNurse.Views
{
    /// <summary>
    /// CountdownWindow.xaml 的交互逻辑
    /// </summary>
    public partial class CountdownWindow : Window
    {
        public CountdownWindow()
        {
            InitializeComponent();
            ContextMenu = App.Menu;
            Loaded += CountdownWindow_Loaded;
            MouseDown += CountdownWindow_MouseDown;
        }

        private void CountdownWindow_MouseDown(object sender, MouseButtonEventArgs e)
        {
            e.Handled = true;
            if (e.LeftButton == MouseButtonState.Pressed)
            {
                DragMove();
            }

            if (e.LeftButton == MouseButtonState.Released)
            {
                var vm = (EyeNurseViewModel)DataContext;
                vm.SaveCountdownWindowPosition(Left, Top);
            }
        }

        private void CountdownWindow_Loaded(object sender, RoutedEventArgs e)
        {
            Loaded -= CountdownWindow_Loaded;

            //处理alt+tab可以看见本程序
            //https://stackoverflow.com/questions/357076/best-way-to-hide-a-window-from-the-alt-tab-program-switcher
            WindowInteropHelper wndHelper = new(this);
            int exStyle = (int)GetWindowLong(wndHelper.Handle, (int)GetWindowLongFields.GWL_EXSTYLE);
            exStyle |= (int)ExtendedWindowStyles.WS_EX_TOOLWINDOW;
            SetWindowLong(wndHelper.Handle, (int)GetWindowLongFields.GWL_EXSTYLE, (IntPtr)exStyle);
        }

        #region Window styles
        [Flags]
        public enum ExtendedWindowStyles
        {
            // ...
            WS_EX_TOOLWINDOW = 0x00000080,
            // ...
        }

        public enum GetWindowLongFields
        {
            // ...
            GWL_EXSTYLE = (-20),
            // ...
        }

        [DllImport("user32.dll")]
        static extern IntPtr GetWindowLong(IntPtr hWnd, int nIndex);

        public static IntPtr SetWindowLong(IntPtr hWnd, int nIndex, IntPtr dwNewLong)
        {
            // Win32 SetWindowLong doesn't clear error on success
            SetLastError(0);

            int error;
            IntPtr result;
            if (IntPtr.Size == 4)
            {
                // use SetWindowLong
                Int32 tempResult = IntSetWindowLong(hWnd, nIndex, IntPtrToInt32(dwNewLong));
                error = Marshal.GetLastWin32Error();
                result = new IntPtr(tempResult);
            }
            else
            {
                // use SetWindowLongPtr
                result = IntSetWindowLongPtr(hWnd, nIndex, dwNewLong);
                error = Marshal.GetLastWin32Error();
            }

            if ((result == IntPtr.Zero) && (error != 0))
            {
                throw new System.ComponentModel.Win32Exception(error);
            }

            return result;
        }

        [DllImport("user32.dll", EntryPoint = "SetWindowLongPtr", SetLastError = true)]
        private static extern IntPtr IntSetWindowLongPtr(IntPtr hWnd, int nIndex, IntPtr dwNewLong);

        [DllImport("user32.dll", EntryPoint = "SetWindowLong", SetLastError = true)]
        private static extern Int32 IntSetWindowLong(IntPtr hWnd, int nIndex, Int32 dwNewLong);

        private static int IntPtrToInt32(IntPtr intPtr)
        {
            return unchecked((int)intPtr.ToInt64());
        }

        [DllImport("kernel32.dll", EntryPoint = "SetLastError")]
        static extern void SetLastError(int dwErrorCode);


        #endregion

        private void Media_Loaded(object sender, RoutedEventArgs e)
        {

        }

        private void TopMost_Checked(object sender, RoutedEventArgs e)
        {
            var vm = (EyeNurseViewModel)DataContext;
            vm.SaveTopMost(vm.TopMost);
        }

        private void TopMost_Unchecked(object sender, RoutedEventArgs e)
        {
            var vm = (EyeNurseViewModel)DataContext;
            vm.SaveTopMost(vm.TopMost);
        }
    }
}
