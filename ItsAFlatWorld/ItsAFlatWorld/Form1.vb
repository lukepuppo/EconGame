Public Class parent
    Dim up As Boolean
    Dim dow As Boolean
    Dim boost As Boolean
    Dim moveBoost As Integer = 4
    Dim moveObjs As Integer = 4
    Dim bankAccount As Double = 100000
    Dim highScore As Double

    Dim objs(14) As PictureBox
    Public Sub New()
        InitializeComponent()
        createArray()

    End Sub

    Private Sub createArray()
        objs(0) = spriteIRS1
        objs(1) = spriteMF1
        objs(2) = spriteIF1
        objs(3) = spriteAF1
        objs(4) = spriteIF2
        objs(5) = spriteMW1
        objs(6) = spriteMF1
        objs(7) = spriteIRS1
        objs(8) = spriteCF1
        objs(9) = spriteMF2
        objs(10) = spriteIF1
        objs(11) = spriteAF1
        objs(12) = spriteIRS1
        objs(13) = spriteAF2
        objs(14) = spriteMF2

    End Sub


    Private Sub gameOver()
        GamePanel.Visible = False
        lblHighScoreVal.Text = highScore
        panelHighScore.Visible = True
    End Sub






    Private Function getRandomStartLocationY()
        Randomize()
        Dim value As Integer = CInt(Int((600 * Rnd()) + 1))
        Return value
    End Function

    Private Sub moveSprites()
        For i = 0 To 14
            If objs(i).Visible = True Then
                objs(i).Left = objs(i).Left - 5
                If objs(i).Bounds.IntersectsWith(spriteDeeder.Bounds()) Then
                    objs(i).Visible = False
                    If i = 0 Or i = 7 Or i = 12 Then
                        lblChanges.Text = bankAccount * -0.3
                        bankAccount = bankAccount * 0.7
                    ElseIf i = 1 Or i = 6 Or i = 9 Or i = 14 Then
                        lblChanges.Text = "+45000"
                        bankAccount = bankAccount + 45000
                    ElseIf i = 2 Or i = 4 Or i = 10 Then
                        lblChanges.Text = "+80000"
                        bankAccount = bankAccount + 80000
                    ElseIf i = 3 Or i = 11 Or i = 13 Then
                        lblChanges.Text = "+20000"
                        bankAccount = bankAccount + 20000
                    ElseIf i = 8 Then
                        lblChanges.Text = "+120000"
                        bankAccount = bankAccount + 125000
                    ElseIf i = 5 Then
                        Randomize()
                        Dim tempValue As Integer = CInt(Int((2 * Rnd()) + 1))
                        If tempValue = 1 Then
                            lblChanges.Text = bankAccount * 5
                            bankAccount = bankAccount * 5
                        ElseIf tempValue = 2 Then
                            lblChanges.Text = 100 - bankAccount
                            bankAccount = 100
                        End If
                        If bankAccount > highScore Then
                            highScore = bankAccount
                        End If
                    End If
                End If

                If objs(i).Right < 0 Then
                    objs(i).Visible = False
                End If
            End If

        Next

    End Sub

    Dim currentSprite As Integer = 0
    Private Sub sendSprite()
        objs(currentSprite).Location = defaultObj.Location
        objs(currentSprite).Top = objs(currentSprite).Top + getRandomStartLocationY()
        objs(currentSprite).Visible = True
        currentSprite = currentSprite + 1
        If currentSprite = 14 Then
            currentSprite = 0
        End If
    End Sub


    Private Sub updateBankAccount()
        lblBankAccount.Text = bankAccount
    End Sub



#Region "buttons"
    Private Sub btnQuit_Click(sender As Object, e As EventArgs) Handles btnQuit.Click
        Me.Close()
    End Sub

    Private Sub btnStart_Click(sender As Object, e As EventArgs) Handles btnStart.Click
        GamePanel.Show()
        Title.Hide()
        btnStart.Hide()
        btnQuit.Hide()
        pictureDeeder.Hide()
        pictureAmericanFactory.Hide()
        pictureMW.Hide()

        timerPrimarySendSprite.Enabled = True
        timerPrimarySendSprite.Start()
        timerSecondarySendSprite.Enabled = True
        timerSecondarySendSprite.Start()
        moveDeeder.Enabled = True
        moveDeeder.Start()
        timerMoveObj.Enabled = True
        timerMoveObj.Start()
    End Sub
#End Region

#Region "Timers"

    Private Sub moveComp_Tick(sender As Object, e As EventArgs) Handles moveDeeder.Tick
        If boost = True Then
            moveBoost = 4
            bankAccount = bankAccount - 100
        End If
        If up = True Then
            If spriteDeeder.Top > -20 Then
                spriteDeeder.Top = spriteDeeder.Top - (4 + moveBoost)
            End If
        End If
        If dow = True Then
            If spriteDeeder.Bottom < 705 Then
                spriteDeeder.Top = spriteDeeder.Top + (4 + moveBoost)
            End If
        End If
        moveBoost = 0
        updateBankAccount()
        If bankAccount < 0 Then
            gameOver()
        End If
    End Sub



    Private Sub objTimer_Tick(sender As Object, e As EventArgs) Handles timerPrimarySendSprite.Tick
        sendSprite()
    End Sub
    Private Sub moveObj_Tick(sender As Object, e As EventArgs) Handles timerMoveObj.Tick
        moveSprites()
    End Sub
    Private Sub timerSecondarySendSprite_Tick(sender As Object, e As EventArgs) Handles timerSecondarySendSprite.Tick
        sendSprite()
    End Sub

#End Region




#Region "Keypresses"
    Private Sub moveCompUp(sender As Object, e As KeyEventArgs) Handles MyBase.KeyDown
        If e.KeyValue = Keys.Up Then
            up = True
        End If
        If e.KeyValue = Keys.Down Then
            dow = True
        End If
        If e.KeyValue = Keys.B Then
            boost = True
        End If
    End Sub

    Private Sub moveCompStop(sender As Object, e As KeyEventArgs) Handles MyBase.KeyUp
        If e.KeyValue = Keys.Up Then
            up = False
        End If
        If e.KeyValue = Keys.Down Then
            dow = False
        End If
        If e.KeyValue = Keys.B Then
            boost = False
        End If

    End Sub









#End Region






End Class
