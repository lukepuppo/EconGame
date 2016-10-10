<Global.Microsoft.VisualBasic.CompilerServices.DesignerGenerated()>
Partial Class parent
    Inherits System.Windows.Forms.Form

    'Form overrides dispose to clean up the component list.
    <System.Diagnostics.DebuggerNonUserCode()>
    Protected Overrides Sub Dispose(ByVal disposing As Boolean)
        Try
            If disposing AndAlso components IsNot Nothing Then
                components.Dispose()
            End If
        Finally
            MyBase.Dispose(disposing)
        End Try
    End Sub

    'Required by the Windows Form Designer
    Private components As System.ComponentModel.IContainer

    'NOTE: The following procedure is required by the Windows Form Designer
    'It can be modified using the Windows Form Designer.  
    'Do not modify it using the code editor.
    <System.Diagnostics.DebuggerStepThrough()>
    Private Sub InitializeComponent()
        Me.components = New System.ComponentModel.Container()
        Dim resources As System.ComponentModel.ComponentResourceManager = New System.ComponentModel.ComponentResourceManager(GetType(parent))
        Me.btnStart = New System.Windows.Forms.Button()
        Me.btnQuit = New System.Windows.Forms.Button()
        Me.Title = New System.Windows.Forms.Label()
        Me.pictureDeeder = New System.Windows.Forms.PictureBox()
        Me.moveDeeder = New System.Windows.Forms.Timer(Me.components)
        Me.pictureMW = New System.Windows.Forms.PictureBox()
        Me.pictureAmericanFactory = New System.Windows.Forms.PictureBox()
        Me.GamePanel = New System.Windows.Forms.Panel()
        Me.panelHighScore = New System.Windows.Forms.Panel()
        Me.lblHighScoreVal = New System.Windows.Forms.Label()
        Me.lblHighScore = New System.Windows.Forms.Label()
        Me.lblChanges = New System.Windows.Forms.Label()
        Me.lblBankAccount = New System.Windows.Forms.Label()
        Me.defaultObj = New System.Windows.Forms.PictureBox()
        Me.spriteDeeder = New System.Windows.Forms.PictureBox()
        Me.spriteMW2 = New System.Windows.Forms.PictureBox()
        Me.spriteMW1 = New System.Windows.Forms.PictureBox()
        Me.spriteIRS2 = New System.Windows.Forms.PictureBox()
        Me.spriteIRS1 = New System.Windows.Forms.PictureBox()
        Me.spriteCF2 = New System.Windows.Forms.PictureBox()
        Me.spriteCF1 = New System.Windows.Forms.PictureBox()
        Me.spriteIF2 = New System.Windows.Forms.PictureBox()
        Me.spriteIF1 = New System.Windows.Forms.PictureBox()
        Me.spriteMF2 = New System.Windows.Forms.PictureBox()
        Me.spriteMF1 = New System.Windows.Forms.PictureBox()
        Me.spriteAF2 = New System.Windows.Forms.PictureBox()
        Me.spriteAF1 = New System.Windows.Forms.PictureBox()
        Me.timerPrimarySendSprite = New System.Windows.Forms.Timer(Me.components)
        Me.timerMoveObj = New System.Windows.Forms.Timer(Me.components)
        Me.timerSecondarySendSprite = New System.Windows.Forms.Timer(Me.components)
        CType(Me.pictureDeeder, System.ComponentModel.ISupportInitialize).BeginInit()
        CType(Me.pictureMW, System.ComponentModel.ISupportInitialize).BeginInit()
        CType(Me.pictureAmericanFactory, System.ComponentModel.ISupportInitialize).BeginInit()
        Me.GamePanel.SuspendLayout()
        Me.panelHighScore.SuspendLayout()
        CType(Me.defaultObj, System.ComponentModel.ISupportInitialize).BeginInit()
        CType(Me.spriteDeeder, System.ComponentModel.ISupportInitialize).BeginInit()
        CType(Me.spriteMW2, System.ComponentModel.ISupportInitialize).BeginInit()
        CType(Me.spriteMW1, System.ComponentModel.ISupportInitialize).BeginInit()
        CType(Me.spriteIRS2, System.ComponentModel.ISupportInitialize).BeginInit()
        CType(Me.spriteIRS1, System.ComponentModel.ISupportInitialize).BeginInit()
        CType(Me.spriteCF2, System.ComponentModel.ISupportInitialize).BeginInit()
        CType(Me.spriteCF1, System.ComponentModel.ISupportInitialize).BeginInit()
        CType(Me.spriteIF2, System.ComponentModel.ISupportInitialize).BeginInit()
        CType(Me.spriteIF1, System.ComponentModel.ISupportInitialize).BeginInit()
        CType(Me.spriteMF2, System.ComponentModel.ISupportInitialize).BeginInit()
        CType(Me.spriteMF1, System.ComponentModel.ISupportInitialize).BeginInit()
        CType(Me.spriteAF2, System.ComponentModel.ISupportInitialize).BeginInit()
        CType(Me.spriteAF1, System.ComponentModel.ISupportInitialize).BeginInit()
        Me.SuspendLayout()
        '
        'btnStart
        '
        Me.btnStart.BackColor = System.Drawing.Color.Coral
        Me.btnStart.Font = New System.Drawing.Font("Orange Grove", 14.25!, System.Drawing.FontStyle.Bold, System.Drawing.GraphicsUnit.Point, CType(0, Byte))
        Me.btnStart.ForeColor = System.Drawing.SystemColors.ControlText
        Me.btnStart.Location = New System.Drawing.Point(520, 325)
        Me.btnStart.Name = "btnStart"
        Me.btnStart.Size = New System.Drawing.Size(200, 87)
        Me.btnStart.TabIndex = 2
        Me.btnStart.Text = "Start Adventure!"
        Me.btnStart.UseVisualStyleBackColor = False
        '
        'btnQuit
        '
        Me.btnQuit.BackColor = System.Drawing.Color.Coral
        Me.btnQuit.Font = New System.Drawing.Font("Orange Grove", 11.25!, System.Drawing.FontStyle.Regular, System.Drawing.GraphicsUnit.Point, CType(0, Byte))
        Me.btnQuit.Location = New System.Drawing.Point(549, 527)
        Me.btnQuit.Name = "btnQuit"
        Me.btnQuit.Size = New System.Drawing.Size(147, 70)
        Me.btnQuit.TabIndex = 1
        Me.btnQuit.Text = "Quit :("
        Me.btnQuit.UseVisualStyleBackColor = False
        '
        'Title
        '
        Me.Title.AutoSize = True
        Me.Title.BackColor = System.Drawing.Color.Coral
        Me.Title.Font = New System.Drawing.Font("Storybook", 51.75!, System.Drawing.FontStyle.Regular, System.Drawing.GraphicsUnit.Point, CType(0, Byte))
        Me.Title.Location = New System.Drawing.Point(148, 49)
        Me.Title.Name = "Title"
        Me.Title.RightToLeft = System.Windows.Forms.RightToLeft.No
        Me.Title.Size = New System.Drawing.Size(970, 80)
        Me.Title.TabIndex = 0
        Me.Title.Text = "It's A Flat World After All"
        '
        'pictureDeeder
        '
        Me.pictureDeeder.BackColor = System.Drawing.Color.Transparent
        Me.pictureDeeder.Image = Global.ItsAFlatWorld.My.Resources.Resources.Deeder
        Me.pictureDeeder.Location = New System.Drawing.Point(35, 239)
        Me.pictureDeeder.Name = "pictureDeeder"
        Me.pictureDeeder.Size = New System.Drawing.Size(304, 348)
        Me.pictureDeeder.SizeMode = System.Windows.Forms.PictureBoxSizeMode.StretchImage
        Me.pictureDeeder.TabIndex = 3
        Me.pictureDeeder.TabStop = False
        '
        'moveDeeder
        '
        Me.moveDeeder.Interval = 5
        '
        'pictureMW
        '
        Me.pictureMW.BackColor = System.Drawing.Color.Transparent
        Me.pictureMW.Image = Global.ItsAFlatWorld.My.Resources.Resources.StockMarket
        Me.pictureMW.Location = New System.Drawing.Point(987, 188)
        Me.pictureMW.Name = "pictureMW"
        Me.pictureMW.Size = New System.Drawing.Size(192, 152)
        Me.pictureMW.SizeMode = System.Windows.Forms.PictureBoxSizeMode.Zoom
        Me.pictureMW.TabIndex = 5
        Me.pictureMW.TabStop = False
        '
        'pictureAmericanFactory
        '
        Me.pictureAmericanFactory.BackColor = System.Drawing.Color.Transparent
        Me.pictureAmericanFactory.Image = Global.ItsAFlatWorld.My.Resources.Resources.American_Factory
        Me.pictureAmericanFactory.Location = New System.Drawing.Point(748, 401)
        Me.pictureAmericanFactory.Name = "pictureAmericanFactory"
        Me.pictureAmericanFactory.Size = New System.Drawing.Size(294, 206)
        Me.pictureAmericanFactory.SizeMode = System.Windows.Forms.PictureBoxSizeMode.Zoom
        Me.pictureAmericanFactory.TabIndex = 6
        Me.pictureAmericanFactory.TabStop = False
        '
        'GamePanel
        '
        Me.GamePanel.BackColor = System.Drawing.SystemColors.MenuHighlight
        Me.GamePanel.Controls.Add(Me.panelHighScore)
        Me.GamePanel.Controls.Add(Me.lblChanges)
        Me.GamePanel.Controls.Add(Me.lblBankAccount)
        Me.GamePanel.Controls.Add(Me.defaultObj)
        Me.GamePanel.Controls.Add(Me.spriteDeeder)
        Me.GamePanel.Controls.Add(Me.spriteMW2)
        Me.GamePanel.Controls.Add(Me.spriteMW1)
        Me.GamePanel.Controls.Add(Me.spriteIRS2)
        Me.GamePanel.Controls.Add(Me.spriteIRS1)
        Me.GamePanel.Controls.Add(Me.spriteCF2)
        Me.GamePanel.Controls.Add(Me.spriteCF1)
        Me.GamePanel.Controls.Add(Me.spriteIF2)
        Me.GamePanel.Controls.Add(Me.spriteIF1)
        Me.GamePanel.Controls.Add(Me.spriteMF2)
        Me.GamePanel.Controls.Add(Me.spriteMF1)
        Me.GamePanel.Controls.Add(Me.spriteAF2)
        Me.GamePanel.Controls.Add(Me.spriteAF1)
        Me.GamePanel.Location = New System.Drawing.Point(0, 0)
        Me.GamePanel.Name = "GamePanel"
        Me.GamePanel.Size = New System.Drawing.Size(2800, 682)
        Me.GamePanel.TabIndex = 8
        Me.GamePanel.Visible = False
        '
        'panelHighScore
        '
        Me.panelHighScore.Controls.Add(Me.lblHighScoreVal)
        Me.panelHighScore.Controls.Add(Me.lblHighScore)
        Me.panelHighScore.Location = New System.Drawing.Point(0, -1)
        Me.panelHighScore.Name = "panelHighScore"
        Me.panelHighScore.Size = New System.Drawing.Size(1267, 683)
        Me.panelHighScore.TabIndex = 18
        Me.panelHighScore.Visible = False
        '
        'lblHighScoreVal
        '
        Me.lblHighScoreVal.AutoSize = True
        Me.lblHighScoreVal.Font = New System.Drawing.Font("Orange Grove", 36.0!, System.Drawing.FontStyle.Bold, System.Drawing.GraphicsUnit.Point, CType(0, Byte))
        Me.lblHighScoreVal.Location = New System.Drawing.Point(552, 383)
        Me.lblHighScoreVal.Name = "lblHighScoreVal"
        Me.lblHighScoreVal.Size = New System.Drawing.Size(0, 98)
        Me.lblHighScoreVal.TabIndex = 1
        '
        'lblHighScore
        '
        Me.lblHighScore.AutoSize = True
        Me.lblHighScore.Font = New System.Drawing.Font("Orange Grove", 48.0!, System.Drawing.FontStyle.Bold, System.Drawing.GraphicsUnit.Point, CType(0, Byte))
        Me.lblHighScore.Location = New System.Drawing.Point(424, 116)
        Me.lblHighScore.Name = "lblHighScore"
        Me.lblHighScore.Size = New System.Drawing.Size(497, 131)
        Me.lblHighScore.TabIndex = 0
        Me.lblHighScore.Text = "High Score:"
        '
        'lblChanges
        '
        Me.lblChanges.Anchor = CType((System.Windows.Forms.AnchorStyles.Top Or System.Windows.Forms.AnchorStyles.Right), System.Windows.Forms.AnchorStyles)
        Me.lblChanges.AutoSize = True
        Me.lblChanges.Font = New System.Drawing.Font("Orange Grove", 14.25!, System.Drawing.FontStyle.Bold, System.Drawing.GraphicsUnit.Point, CType(0, Byte))
        Me.lblChanges.Location = New System.Drawing.Point(659, 66)
        Me.lblChanges.Name = "lblChanges"
        Me.lblChanges.Size = New System.Drawing.Size(42, 39)
        Me.lblChanges.TabIndex = 17
        Me.lblChanges.Text = "+0"
        Me.lblChanges.TextAlign = System.Drawing.ContentAlignment.MiddleRight
        '
        'lblBankAccount
        '
        Me.lblBankAccount.Anchor = CType((System.Windows.Forms.AnchorStyles.Top Or System.Windows.Forms.AnchorStyles.Right), System.Windows.Forms.AnchorStyles)
        Me.lblBankAccount.AutoSize = True
        Me.lblBankAccount.Font = New System.Drawing.Font("Orange Grove", 27.75!, System.Drawing.FontStyle.Bold, System.Drawing.GraphicsUnit.Point, CType(0, Byte))
        Me.lblBankAccount.Location = New System.Drawing.Point(614, -1)
        Me.lblBankAccount.Name = "lblBankAccount"
        Me.lblBankAccount.Size = New System.Drawing.Size(166, 76)
        Me.lblBankAccount.TabIndex = 16
        Me.lblBankAccount.Text = "100000"
        Me.lblBankAccount.TextAlign = System.Drawing.ContentAlignment.MiddleLeft
        '
        'defaultObj
        '
        Me.defaultObj.Location = New System.Drawing.Point(1280, 0)
        Me.defaultObj.Name = "defaultObj"
        Me.defaultObj.Size = New System.Drawing.Size(100, 50)
        Me.defaultObj.TabIndex = 15
        Me.defaultObj.TabStop = False
        '
        'spriteDeeder
        '
        Me.spriteDeeder.BackColor = System.Drawing.SystemColors.MenuHighlight
        Me.spriteDeeder.BackgroundImageLayout = System.Windows.Forms.ImageLayout.None
        Me.spriteDeeder.Image = Global.ItsAFlatWorld.My.Resources.Resources.Deeder
        Me.spriteDeeder.Location = New System.Drawing.Point(6, 226)
        Me.spriteDeeder.Name = "spriteDeeder"
        Me.spriteDeeder.Size = New System.Drawing.Size(175, 216)
        Me.spriteDeeder.SizeMode = System.Windows.Forms.PictureBoxSizeMode.Zoom
        Me.spriteDeeder.TabIndex = 2
        Me.spriteDeeder.TabStop = False
        '
        'spriteMW2
        '
        Me.spriteMW2.BackColor = System.Drawing.SystemColors.MenuHighlight
        Me.spriteMW2.Image = CType(resources.GetObject("spriteMW2.Image"), System.Drawing.Image)
        Me.spriteMW2.Location = New System.Drawing.Point(1261, 346)
        Me.spriteMW2.Name = "spriteMW2"
        Me.spriteMW2.Size = New System.Drawing.Size(126, 123)
        Me.spriteMW2.SizeMode = System.Windows.Forms.PictureBoxSizeMode.Zoom
        Me.spriteMW2.TabIndex = 14
        Me.spriteMW2.TabStop = False
        Me.spriteMW2.Visible = False
        '
        'spriteMW1
        '
        Me.spriteMW1.BackColor = System.Drawing.SystemColors.MenuHighlight
        Me.spriteMW1.Image = CType(resources.GetObject("spriteMW1.Image"), System.Drawing.Image)
        Me.spriteMW1.Location = New System.Drawing.Point(1261, 217)
        Me.spriteMW1.Name = "spriteMW1"
        Me.spriteMW1.Size = New System.Drawing.Size(126, 123)
        Me.spriteMW1.SizeMode = System.Windows.Forms.PictureBoxSizeMode.Zoom
        Me.spriteMW1.TabIndex = 13
        Me.spriteMW1.TabStop = False
        Me.spriteMW1.Visible = False
        '
        'spriteIRS2
        '
        Me.spriteIRS2.BackColor = System.Drawing.SystemColors.MenuHighlight
        Me.spriteIRS2.Image = CType(resources.GetObject("spriteIRS2.Image"), System.Drawing.Image)
        Me.spriteIRS2.Location = New System.Drawing.Point(1569, 555)
        Me.spriteIRS2.Name = "spriteIRS2"
        Me.spriteIRS2.Size = New System.Drawing.Size(126, 123)
        Me.spriteIRS2.SizeMode = System.Windows.Forms.PictureBoxSizeMode.Zoom
        Me.spriteIRS2.TabIndex = 12
        Me.spriteIRS2.TabStop = False
        Me.spriteIRS2.Visible = False
        '
        'spriteIRS1
        '
        Me.spriteIRS1.BackColor = System.Drawing.SystemColors.MenuHighlight
        Me.spriteIRS1.Image = CType(resources.GetObject("spriteIRS1.Image"), System.Drawing.Image)
        Me.spriteIRS1.Location = New System.Drawing.Point(1569, 426)
        Me.spriteIRS1.Name = "spriteIRS1"
        Me.spriteIRS1.Size = New System.Drawing.Size(126, 123)
        Me.spriteIRS1.SizeMode = System.Windows.Forms.PictureBoxSizeMode.Zoom
        Me.spriteIRS1.TabIndex = 11
        Me.spriteIRS1.TabStop = False
        Me.spriteIRS1.Visible = False
        '
        'spriteCF2
        '
        Me.spriteCF2.BackColor = System.Drawing.SystemColors.MenuHighlight
        Me.spriteCF2.Image = Global.ItsAFlatWorld.My.Resources.Resources.Chinese_Factory
        Me.spriteCF2.Location = New System.Drawing.Point(1569, 297)
        Me.spriteCF2.Name = "spriteCF2"
        Me.spriteCF2.Size = New System.Drawing.Size(126, 123)
        Me.spriteCF2.SizeMode = System.Windows.Forms.PictureBoxSizeMode.Zoom
        Me.spriteCF2.TabIndex = 10
        Me.spriteCF2.TabStop = False
        Me.spriteCF2.Visible = False
        '
        'spriteCF1
        '
        Me.spriteCF1.BackColor = System.Drawing.SystemColors.MenuHighlight
        Me.spriteCF1.Image = Global.ItsAFlatWorld.My.Resources.Resources.Chinese_Factory
        Me.spriteCF1.Location = New System.Drawing.Point(1569, 168)
        Me.spriteCF1.Name = "spriteCF1"
        Me.spriteCF1.Size = New System.Drawing.Size(126, 123)
        Me.spriteCF1.SizeMode = System.Windows.Forms.PictureBoxSizeMode.Zoom
        Me.spriteCF1.TabIndex = 9
        Me.spriteCF1.TabStop = False
        Me.spriteCF1.Visible = False
        '
        'spriteIF2
        '
        Me.spriteIF2.BackColor = System.Drawing.SystemColors.MenuHighlight
        Me.spriteIF2.Image = Global.ItsAFlatWorld.My.Resources.Resources.India_Factory
        Me.spriteIF2.Location = New System.Drawing.Point(1569, 39)
        Me.spriteIF2.Name = "spriteIF2"
        Me.spriteIF2.Size = New System.Drawing.Size(126, 123)
        Me.spriteIF2.SizeMode = System.Windows.Forms.PictureBoxSizeMode.Zoom
        Me.spriteIF2.TabIndex = 8
        Me.spriteIF2.TabStop = False
        Me.spriteIF2.Visible = False
        '
        'spriteIF1
        '
        Me.spriteIF1.BackColor = System.Drawing.SystemColors.MenuHighlight
        Me.spriteIF1.Image = Global.ItsAFlatWorld.My.Resources.Resources.India_Factory
        Me.spriteIF1.Location = New System.Drawing.Point(1432, 555)
        Me.spriteIF1.Name = "spriteIF1"
        Me.spriteIF1.Size = New System.Drawing.Size(126, 123)
        Me.spriteIF1.SizeMode = System.Windows.Forms.PictureBoxSizeMode.Zoom
        Me.spriteIF1.TabIndex = 7
        Me.spriteIF1.TabStop = False
        Me.spriteIF1.Visible = False
        '
        'spriteMF2
        '
        Me.spriteMF2.BackColor = System.Drawing.SystemColors.MenuHighlight
        Me.spriteMF2.Image = Global.ItsAFlatWorld.My.Resources.Resources.MexicoFactory
        Me.spriteMF2.Location = New System.Drawing.Point(1432, 426)
        Me.spriteMF2.Name = "spriteMF2"
        Me.spriteMF2.Size = New System.Drawing.Size(126, 123)
        Me.spriteMF2.SizeMode = System.Windows.Forms.PictureBoxSizeMode.Zoom
        Me.spriteMF2.TabIndex = 6
        Me.spriteMF2.TabStop = False
        Me.spriteMF2.Visible = False
        '
        'spriteMF1
        '
        Me.spriteMF1.BackColor = System.Drawing.SystemColors.MenuHighlight
        Me.spriteMF1.Image = Global.ItsAFlatWorld.My.Resources.Resources.MexicoFactory
        Me.spriteMF1.Location = New System.Drawing.Point(1432, 297)
        Me.spriteMF1.Name = "spriteMF1"
        Me.spriteMF1.Size = New System.Drawing.Size(126, 123)
        Me.spriteMF1.SizeMode = System.Windows.Forms.PictureBoxSizeMode.Zoom
        Me.spriteMF1.TabIndex = 5
        Me.spriteMF1.TabStop = False
        Me.spriteMF1.Visible = False
        '
        'spriteAF2
        '
        Me.spriteAF2.BackColor = System.Drawing.SystemColors.MenuHighlight
        Me.spriteAF2.Image = Global.ItsAFlatWorld.My.Resources.Resources.American_Factory
        Me.spriteAF2.Location = New System.Drawing.Point(1432, 168)
        Me.spriteAF2.Name = "spriteAF2"
        Me.spriteAF2.Size = New System.Drawing.Size(126, 123)
        Me.spriteAF2.SizeMode = System.Windows.Forms.PictureBoxSizeMode.Zoom
        Me.spriteAF2.TabIndex = 4
        Me.spriteAF2.TabStop = False
        Me.spriteAF2.Visible = False
        '
        'spriteAF1
        '
        Me.spriteAF1.BackColor = System.Drawing.SystemColors.MenuHighlight
        Me.spriteAF1.Image = Global.ItsAFlatWorld.My.Resources.Resources.American_Factory
        Me.spriteAF1.Location = New System.Drawing.Point(1432, 39)
        Me.spriteAF1.Name = "spriteAF1"
        Me.spriteAF1.Size = New System.Drawing.Size(126, 123)
        Me.spriteAF1.SizeMode = System.Windows.Forms.PictureBoxSizeMode.Zoom
        Me.spriteAF1.TabIndex = 3
        Me.spriteAF1.TabStop = False
        Me.spriteAF1.Visible = False
        '
        'timerPrimarySendSprite
        '
        Me.timerPrimarySendSprite.Interval = 2500
        '
        'timerMoveObj
        '
        Me.timerMoveObj.Interval = 10
        '
        'timerSecondarySendSprite
        '
        Me.timerSecondarySendSprite.Interval = 4500
        '
        'parent
        '
        Me.AutoScaleDimensions = New System.Drawing.SizeF(6.0!, 13.0!)
        Me.AutoScaleMode = System.Windows.Forms.AutoScaleMode.Font
        Me.BackgroundImage = Global.ItsAFlatWorld.My.Resources.Resources.smallworld
        Me.BackgroundImageLayout = System.Windows.Forms.ImageLayout.Stretch
        Me.ClientSize = New System.Drawing.Size(1264, 681)
        Me.Controls.Add(Me.GamePanel)
        Me.Controls.Add(Me.pictureAmericanFactory)
        Me.Controls.Add(Me.pictureMW)
        Me.Controls.Add(Me.pictureDeeder)
        Me.Controls.Add(Me.btnQuit)
        Me.Controls.Add(Me.btnStart)
        Me.Controls.Add(Me.Title)
        Me.KeyPreview = True
        Me.Name = "parent"
        Me.Text = "It's A Flat World"
        CType(Me.pictureDeeder, System.ComponentModel.ISupportInitialize).EndInit()
        CType(Me.pictureMW, System.ComponentModel.ISupportInitialize).EndInit()
        CType(Me.pictureAmericanFactory, System.ComponentModel.ISupportInitialize).EndInit()
        Me.GamePanel.ResumeLayout(False)
        Me.GamePanel.PerformLayout()
        Me.panelHighScore.ResumeLayout(False)
        Me.panelHighScore.PerformLayout()
        CType(Me.defaultObj, System.ComponentModel.ISupportInitialize).EndInit()
        CType(Me.spriteDeeder, System.ComponentModel.ISupportInitialize).EndInit()
        CType(Me.spriteMW2, System.ComponentModel.ISupportInitialize).EndInit()
        CType(Me.spriteMW1, System.ComponentModel.ISupportInitialize).EndInit()
        CType(Me.spriteIRS2, System.ComponentModel.ISupportInitialize).EndInit()
        CType(Me.spriteIRS1, System.ComponentModel.ISupportInitialize).EndInit()
        CType(Me.spriteCF2, System.ComponentModel.ISupportInitialize).EndInit()
        CType(Me.spriteCF1, System.ComponentModel.ISupportInitialize).EndInit()
        CType(Me.spriteIF2, System.ComponentModel.ISupportInitialize).EndInit()
        CType(Me.spriteIF1, System.ComponentModel.ISupportInitialize).EndInit()
        CType(Me.spriteMF2, System.ComponentModel.ISupportInitialize).EndInit()
        CType(Me.spriteMF1, System.ComponentModel.ISupportInitialize).EndInit()
        CType(Me.spriteAF2, System.ComponentModel.ISupportInitialize).EndInit()
        CType(Me.spriteAF1, System.ComponentModel.ISupportInitialize).EndInit()
        Me.ResumeLayout(False)
        Me.PerformLayout()

    End Sub
    Friend WithEvents Title As Label
    Friend WithEvents pictureDeeder As PictureBox
    Friend WithEvents btnStart As Button
    Friend WithEvents btnQuit As Button
    Friend WithEvents moveDeeder As Timer
    Friend WithEvents pictureMW As PictureBox
    Friend WithEvents pictureAmericanFactory As PictureBox
    Friend WithEvents GamePanel As Panel
    Friend WithEvents spriteDeeder As PictureBox
    Friend WithEvents spriteMW2 As PictureBox
    Friend WithEvents spriteMW1 As PictureBox
    Friend WithEvents spriteIRS2 As PictureBox
    Friend WithEvents spriteIRS1 As PictureBox
    Friend WithEvents spriteCF2 As PictureBox
    Friend WithEvents spriteCF1 As PictureBox
    Friend WithEvents spriteIF2 As PictureBox
    Friend WithEvents spriteIF1 As PictureBox
    Friend WithEvents spriteMF2 As PictureBox
    Friend WithEvents spriteMF1 As PictureBox
    Friend WithEvents spriteAF2 As PictureBox
    Friend WithEvents spriteAF1 As PictureBox
    Friend WithEvents timerPrimarySendSprite As Timer
    Friend WithEvents timerMoveObj As Timer
    Friend WithEvents defaultObj As PictureBox
    Friend WithEvents timerSecondarySendSprite As Timer
    Friend WithEvents lblBankAccount As Label
    Friend WithEvents lblChanges As Label
    Friend WithEvents panelHighScore As Panel
    Friend WithEvents lblHighScore As Label
    Friend WithEvents lblHighScoreVal As Label
End Class
