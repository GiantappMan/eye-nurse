using System;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Windows.Data;
using System.Windows.Media;

namespace EyeNurse.Client.Views.Converters
{
    public class TimeSpanToColorBrushConveter : IValueConverter
    {
        public Color From { get; set; }
        public Color To { get; set; }
        public int DurationSecond { get; set; }
        public object Convert(object value, Type targetType, object parameter, CultureInfo culture)
        {
            if (!(value is TimeSpan))
                return value;

            var time = (TimeSpan)value;
            double percent = time.TotalSeconds / DurationSecond;
            if (percent == 0)
                percent = 0.1d;
            if (percent > 1)
                percent = 1;
            byte r = (byte)(From.R + (To.R - From.R) * percent);
            byte g = (byte)(From.G + (To.G - From.G) * percent);
            byte b = (byte)(From.B + (To.B - From.B) * percent);
            byte a = (byte)(From.A + (To.A - From.A) * percent);
            var color = Color.FromArgb(a, r, g, b);
            return new SolidColorBrush(color);
        }

        public object ConvertBack(object value, Type targetType, object parameter, CultureInfo culture)
        {
            throw new NotImplementedException();
        }
    }
}
