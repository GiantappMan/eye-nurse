using Caliburn.Micro;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace EyeNurse.Client.ViewModels
{
    public class ContextMenuViewModel : Screen
    {
        public ContextMenuViewModel()
        {

        }

        #region properties

        #region Desc

        /// <summary>
        /// The <see cref="Desc" /> property's name.
        /// </summary>
        public const string DescPropertyName = "Desc";

        private string _Desc = "tes\r\ntsetst";

        /// <summary>
        /// Desc
        /// </summary>
        public string Desc
        {
            get { return _Desc; }

            set
            {
                if (_Desc == value) return;

                _Desc = value;
                NotifyOfPropertyChange(DescPropertyName);
            }
        }

        #endregion

        #endregion

        #region methods

        public void ExitApp()
        {

        }

        #endregion

        #region commands

        //#region ExitAppCommand

        //private DelegateCommand _ExitAppCommand;

        ///// <summary>
        ///// Gets the ExitAppCommand.
        ///// </summary>
        //public DelegateCommand ExitAppCommand
        //{
        //    get
        //    {
        //        return _ExitAppCommand ?? (_ExitAppCommand = new DelegateCommand(ExecuteExitAppCommand));
        //    }
        //}

        //private void ExecuteExitAppCommand()
        //{

        //}

        //#endregion

        #endregion
    }
}
