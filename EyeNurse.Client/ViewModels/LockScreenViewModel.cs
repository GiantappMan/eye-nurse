using Caliburn.Micro;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace EyeNurse.Client.ViewModels
{
    public class LockScreenViewModel : Screen
    {
        #region public methods

        public void Exit()
        {
            TryClose();
        }
        #endregion
    }
}
