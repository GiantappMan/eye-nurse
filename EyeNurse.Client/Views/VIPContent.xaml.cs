using Caliburn.Micro;
using EyeNurse.Client.Services;
using EyeNurse.Client.ViewModels;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Data;
using System.Windows.Documents;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using System.Windows.Navigation;
using System.Windows.Shapes;

namespace EyeNurse.Client.Views
{
    /// <summary>
    /// Interaction logic for VIPContent.xaml
    /// </summary>
    public partial class VIPContent : UserControl
    {
        public VIPContent(string text)
        {
            InitializeComponent();
            txt.Text = text;
        }

        private void BtnQQGroup_Click(object sender, RoutedEventArgs e)
        {
            IoC.Get<EyeNurseService>().OpenVIPQQGroupLink();
        }

        private void BtnCopy_Click(object sender, RoutedEventArgs e)
        {
            IoC.Get<EyeNurseService>().CopyVipContent();
        }

        private void BtnClose_Click(object sender, RoutedEventArgs e)
        {
            Window window = Window.GetWindow(this);
            window?.Close();
        }
    }
}
