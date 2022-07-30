using EyeNurse.Services;
using NUnit.Framework;
using System;
using System.Threading.Tasks;

namespace TestProject
{
    public class GlobalTimerServiceTest
    {
        [SetUp]
        public void Setup()
        {
        }

        [Test]
        public async Task Start()
        {
            GlobalTimerService service;
            try
            {
                service = new(TimeSpan.FromSeconds(0.1));
                service.Start();
            }
            catch (Exception ex)
            {
                Assert.IsTrue(ex is ArgumentOutOfRangeException);
            }

            async Task TestTimer(int seconds)
            {
                service = new(TimeSpan.FromSeconds(1));
                //触发统计
                int count = 0;
                void Service_OnTick(object? sender, TimerTriggerEventArgs e)
                {
                    ++count;
                };
                service.Trigger += Service_OnTick;
                Assert.IsTrue(service.Start());
                Assert.IsFalse(service.Start());
                await Task.Delay(seconds * 1000);
                //1，2秒误差
                Assert.IsTrue(count >= seconds - 2 && count <= seconds + 2);

                service.Trigger -= Service_OnTick;

                service.Stop();

                count = 0;
                Assert.IsTrue(service.Start());
                Assert.AreEqual(count, 0);
            }

            await TestTimer(6);
            await TestTimer(8);
            await TestTimer(3);
            Assert.Pass();
        }

        [Test]
        public void Stop()
        {
            GlobalTimerService service = new(TimeSpan.FromMinutes(1));
            Assert.IsFalse(service.Stop());
            Assert.IsTrue(service.Start());
            Assert.IsTrue(service.Stop());
            Assert.IsFalse(service.Stop());
            Assert.Pass();
        }

        [Test]
        public async Task Pause()
        {
            GlobalTimerService service = new(TimeSpan.FromSeconds(1));
            int count = 0;
            void Service_OnTick(object? sender, TimerTriggerEventArgs e)
            {
                ++count;
            };
            service.Trigger += Service_OnTick;
            int seconds = 1;
            //未开始
            Assert.IsFalse(service.Pause());
            Assert.IsTrue(service.Start());
            await Task.Delay(seconds * 1000);
            //1，2秒误差
            Assert.IsTrue(count >= seconds - 2 && count <= seconds + 2);
            Assert.IsTrue(service.Pause());
            //已暂停
            Assert.IsFalse(service.Pause());

            await Task.Delay(3000);
            //判断没有增加
            Assert.IsTrue(count >= seconds - 2 && count <= seconds + 2);

            service.Trigger -= Service_OnTick;
            service.Stop();
            Assert.Pass();
        }

        [Test]
        public async Task Resume()
        {
            GlobalTimerService service = new(TimeSpan.FromSeconds(1));
            int count = 0;
            void Service_OnTick(object? sender, TimerTriggerEventArgs e)
            {
                ++count;
            };
            service.Trigger += Service_OnTick;
            int seconds = 5;
            //未开始
            Assert.IsTrue(service.Start());
            Assert.IsTrue(service.Pause());
            Assert.True(service.Resume());
            await Task.Delay(seconds * 1000);

            //已恢复
            Assert.IsFalse(service.Resume());

            //1，2秒误差
            Assert.IsTrue(count >= seconds - 2 && count <= seconds + 2);
            Assert.IsTrue(service.Pause());
            //已暂停
            Assert.IsFalse(service.Pause());

            await Task.Delay(3000);
            //判断没有增加
            Assert.IsTrue(count >= seconds - 2 && count <= seconds + 2);

            service.Trigger -= Service_OnTick;
            service.Stop();
            Assert.Pass();
        }

        TimeSpan? lastRemainingTime = null;
        [Test]
        public async Task Reset()
        {
            object lastRemainingTimeLock = new();
            TimeSpan triggerTs = TimeSpan.FromMinutes(2);
            GlobalTimerService service = new(triggerTs);
            int count = 0;
            void Service_Elapsed(object? sender, TimerElapsedEventArgs e)
            {
                System.Diagnostics.Debug.WriteLine(e.RemainingTime);
                count++;
                Assert.IsNotNull(e.RemainingTime);
                Assert.IsTrue(e.RemainingTime.TotalSeconds >= 0);
                lock (lastRemainingTimeLock)
                {
                    if (lastRemainingTime != null)
                        Assert.IsTrue(lastRemainingTime > e.RemainingTime);
                    lastRemainingTime = e.RemainingTime;
                }
            }
            service.Elapsed += Service_Elapsed;
            service.Start();
            int seconds = 10;
            await Task.Delay(seconds * 1000);
            var tmpTs = triggerTs - lastRemainingTime;
            //误差1s
            Assert.IsTrue(tmpTs < TimeSpan.FromSeconds(seconds + 1));

            lock (lastRemainingTimeLock)
            {
                service.Reset();
                lastRemainingTime = null;
                System.Diagnostics.Debug.WriteLine($"Reset:{lastRemainingTime}");
            }

            seconds = 5;
            await Task.Delay(seconds * 1000);
            tmpTs = triggerTs - lastRemainingTime;
            //误差1s
            Assert.IsTrue(tmpTs < TimeSpan.FromSeconds(seconds + 1));

            service.Elapsed -= Service_Elapsed;
            service.Stop();

            Assert.IsTrue(count >= 0);
            Assert.Pass();
        }
    }
}