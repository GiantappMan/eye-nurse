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
using System.Windows.Shapes;

namespace EyeNurse.Client.Views
{
    /// <summary>
    /// Interaction logic for CountDownView.xaml
    /// </summary>
    public partial class CountDownView : Window
    {
        public CountDownView()
        {
            InitializeComponent();
            MouseDown += CountDownView_MouseDown;
            MouseUp += CountDownView_MouseUp;
        }

        private void CountDownView_MouseUp(object sender, MouseButtonEventArgs e)
        {
            if (e.ChangedButton == MouseButton.Left)
            {
                var vm = DataContext as CountDownViewModel;
                vm.SavePosition();
            }
        }

        private void CountDownView_MouseDown(object sender, MouseButtonEventArgs e)
        {
            if (e.ChangedButton == MouseButton.Left)
                DragMove();
        }
    }
}
