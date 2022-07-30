using System;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Windows.Data;

namespace EyeNurse.Views.Converters
{
    internal class TimeSpanToPercent : IMultiValueConverter
    {
        public object Convert(object[] values, Type targetType, object parameter, CultureInfo culture)
        {
            if (values.Length < 2 || values[0] == null || values[1] == null)
                return 100.0;

            _ = TimeSpan.TryParse(values[0].ToString(), out var current);
            _ = TimeSpan.TryParse(values[1].ToString(), out var interval);

            var percent = current.TotalSeconds / interval.TotalSeconds * 100;
            return percent;
        }

        public object[] ConvertBack(object value, Type[] targetTypes, object parameter, CultureInfo culture)
        {
            throw new NotImplementedException();
        }
    }
}
